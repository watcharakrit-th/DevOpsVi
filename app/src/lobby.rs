//! Lobbies: two seats, one XO game at a time, and rematches. Pure like the
//! game rules; the Redis store and the HTTP handlers wrap it.

use std::fmt::{self, Write};

use serde::{Deserialize, Serialize, Serializer};
use subtle::ConstantTimeEq;

use crate::games::xo::{Game, Mark, MoveError, Status};

/// The characters lobby codes are made of. 0/O, 1/I and L are left out
/// because they are easy to mix up when a code is read out or typed.
pub const CODE_ALPHABET: &[u8; 31] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const CODE_LEN: usize = 6;

/// A lobby's 6-character code, as used in the invite link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobbyCode(String);

impl LobbyCode {
    pub fn random() -> Self {
        // A byte at or above 248 (8 × 31) is skipped: mapping it with `% 31`
        // would make the first characters of the alphabet more likely.
        const LIMIT: u8 = (256 / CODE_ALPHABET.len() * CODE_ALPHABET.len()) as u8;
        let mut code = String::with_capacity(CODE_LEN);
        while code.len() < CODE_LEN {
            let [byte] = random_bytes::<1>();
            if byte < LIMIT {
                code.push(char::from(
                    CODE_ALPHABET[usize::from(byte) % CODE_ALPHABET.len()],
                ));
            }
        }
        Self(code)
    }

    /// Accepts a code in either case. Anything that isn't 6 characters from
    /// the alphabet is `None`.
    pub fn parse(input: &str) -> Option<Self> {
        if input.len() != CODE_LEN {
            return None;
        }
        let code = input.to_ascii_uppercase();
        code.bytes()
            .all(|b| CODE_ALPHABET.contains(&b))
            .then_some(Self(code))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LobbyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The secret that proves a seat: 128 random bits as 32 hex characters.
/// Its `Debug` output is redacted so it can't end up in a log line.
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Token(String);

impl Token {
    pub fn random() -> Self {
        let mut hex = String::with_capacity(32);
        for byte in random_bytes::<16>() {
            write!(hex, "{byte:02x}").expect("writing to a String can't fail");
        }
        Self(hex)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Compares in constant time, so response times can't reveal how much
    /// of a guessed token was right.
    pub fn matches(&self, presented: &str) -> bool {
        self.0.as_bytes().ct_eq(presented.as_bytes()).into()
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(redacted)")
    }
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the OS random number generator works");
    bytes
}

/// Seat 1 belongs to whoever created the lobby, seat 2 to whoever joined.
/// The API shows seats as the numbers 1 and 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
pub enum Seat {
    One,
    Two,
}

impl Seat {
    pub fn other(self) -> Self {
        match self {
            Seat::One => Seat::Two,
            Seat::Two => Seat::One,
        }
    }

    fn index(self) -> usize {
        match self {
            Seat::One => 0,
            Seat::Two => 1,
        }
    }
}

impl From<Seat> for u8 {
    fn from(seat: Seat) -> Self {
        match seat {
            Seat::One => 1,
            Seat::Two => 2,
        }
    }
}

impl TryFrom<u8> for Seat {
    type Error = String;

    fn try_from(number: u8) -> Result<Self, Self::Error> {
        match number {
            1 => Ok(Seat::One),
            2 => Ok(Seat::Two),
            _ => Err(format!("there is no seat {number}")),
        }
    }
}

/// How a lobby stands, as the API reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyStatus {
    Waiting,
    Playing,
    XWon,
    OWon,
    Draw,
}

impl LobbyStatus {
    /// The API name, which is also the `result` label on the games-finished
    /// metric.
    pub fn as_str(self) -> &'static str {
        match self {
            LobbyStatus::Waiting => "waiting",
            LobbyStatus::Playing => "playing",
            LobbyStatus::XWon => "x_won",
            LobbyStatus::OWon => "o_won",
            LobbyStatus::Draw => "draw",
        }
    }

    pub fn is_finished(self) -> bool {
        matches!(
            self,
            LobbyStatus::XWon | LobbyStatus::OWon | LobbyStatus::Draw
        )
    }
}

impl Serialize for LobbyStatus {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyError {
    /// Both seats are taken.
    LobbyFull,
    /// Seat 2 is still empty.
    WaitingForOpponent,
    /// A rematch needs a won or drawn game.
    GameNotOver,
    /// The game rules refused the move.
    Move(MoveError),
}

/// Everything stored for one lobby: its whole Redis value.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lobby {
    /// Goes up by one on every change. Clients send back the version they
    /// saw, so a click on an out-of-date board is refused.
    version: u64,
    /// Indexed by seat. Seat 2 is empty until someone joins.
    seats: [Option<Token>; 2],
    /// The seat playing X in the current game. X always moves first.
    x_seat: Seat,
    game: Game,
}

impl Lobby {
    /// The creator takes seat 1 and plays X in the first game.
    pub fn new(creator: Token) -> Self {
        Self {
            version: 1,
            seats: [Some(creator), None],
            x_seat: Seat::One,
            game: Game::new(),
        }
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// The seat `presented` is the token for. Both seats are compared every
    /// time.
    pub fn seat_of(&self, presented: &str) -> Option<Seat> {
        let matches = self
            .seats
            .each_ref()
            .map(|seat| seat.as_ref().is_some_and(|token| token.matches(presented)));
        match matches {
            [true, _] => Some(Seat::One),
            [_, true] => Some(Seat::Two),
            _ => None,
        }
    }

    pub fn join(&mut self, token: Token) -> Result<(), LobbyError> {
        let seat = &mut self.seats[Seat::Two.index()];
        if seat.is_some() {
            return Err(LobbyError::LobbyFull);
        }
        *seat = Some(token);
        self.version += 1;
        Ok(())
    }

    /// Plays `cell` for `seat` and returns how the lobby stands afterwards.
    pub fn play(&mut self, seat: Seat, cell: usize) -> Result<LobbyStatus, LobbyError> {
        if !self.full() {
            return Err(LobbyError::WaitingForOpponent);
        }
        let mark = if seat == self.x_seat {
            Mark::X
        } else {
            Mark::O
        };
        self.game.play(mark, cell).map_err(LobbyError::Move)?;
        self.version += 1;
        Ok(self.status())
    }

    /// Starts a new game after a win or draw. X and O swap seats.
    pub fn rematch(&mut self) -> Result<(), LobbyError> {
        if !self.status().is_finished() {
            return Err(LobbyError::GameNotOver);
        }
        self.x_seat = self.x_seat.other();
        self.game = Game::new();
        self.version += 1;
        Ok(())
    }

    pub fn status(&self) -> LobbyStatus {
        if !self.full() {
            return LobbyStatus::Waiting;
        }
        match self.game.status() {
            Status::Playing => LobbyStatus::Playing,
            Status::XWon => LobbyStatus::XWon,
            Status::OWon => LobbyStatus::OWon,
            Status::Draw => LobbyStatus::Draw,
        }
    }

    /// What anyone with the link sees. Never includes tokens.
    pub fn view(&self) -> LobbyView {
        let status = self.status();
        LobbyView {
            board: *self.game.cells(),
            status,
            turn: (status == LobbyStatus::Playing)
                .then(|| self.game.turn())
                .flatten(),
            x_seat: self.x_seat,
            version: self.version,
        }
    }

    fn full(&self) -> bool {
        self.seats[Seat::Two.index()].is_some()
    }
}

/// A lobby as `GET /api/lobbies/{code}` returns it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LobbyView {
    pub board: [Option<Mark>; 9],
    pub status: LobbyStatus,
    /// Whose turn it is; null unless a game is being played.
    pub turn: Option<Mark>,
    pub x_seat: Seat,
    pub version: u64,
}
