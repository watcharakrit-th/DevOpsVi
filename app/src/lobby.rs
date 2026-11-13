//! Lobbies: two seats and one XO game at a time. Stub for the failing tests.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::games::xo::{Game, Mark, MoveError};

pub const CODE_ALPHABET: &[u8; 31] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobbyCode(String);

impl LobbyCode {
    pub fn random() -> Self {
        todo!()
    }

    pub fn parse(_input: &str) -> Option<Self> {
        todo!()
    }

    pub fn as_str(&self) -> &str {
        todo!()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Token(String);

impl Token {
    pub fn random() -> Self {
        todo!()
    }

    pub fn as_str(&self) -> &str {
        todo!()
    }

    pub fn matches(&self, _presented: &str) -> bool {
        todo!()
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
pub enum Seat {
    One,
    Two,
}

impl Seat {
    pub fn other(self) -> Self {
        todo!()
    }
}

impl From<Seat> for u8 {
    fn from(_seat: Seat) -> Self {
        todo!()
    }
}

impl TryFrom<u8> for Seat {
    type Error = String;

    fn try_from(_number: u8) -> Result<Self, Self::Error> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyStatus {
    Waiting,
    Playing,
    XWon,
    OWon,
    Draw,
}

impl LobbyStatus {
    pub fn as_str(self) -> &'static str {
        todo!()
    }
}

impl Serialize for LobbyStatus {
    fn serialize<S: Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyError {
    LobbyFull,
    WaitingForOpponent,
    GameNotOver,
    Move(MoveError),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lobby {
    version: u64,
    seats: [Option<Token>; 2],
    x_seat: Seat,
    game: Game,
}

impl Lobby {
    pub fn new(_creator: Token) -> Self {
        todo!()
    }

    pub fn version(&self) -> u64 {
        todo!()
    }

    pub fn seat_of(&self, _presented: &str) -> Option<Seat> {
        todo!()
    }

    pub fn join(&mut self, _token: Token) -> Result<(), LobbyError> {
        todo!()
    }

    pub fn play(&mut self, _seat: Seat, _cell: usize) -> Result<LobbyStatus, LobbyError> {
        todo!()
    }

    pub fn rematch(&mut self) -> Result<(), LobbyError> {
        todo!()
    }

    pub fn view(&self) -> LobbyView {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LobbyView {
    pub board: [Option<Mark>; 9],
    pub status: LobbyStatus,
    pub turn: Option<Mark>,
    pub x_seat: Seat,
    pub version: u64,
}
