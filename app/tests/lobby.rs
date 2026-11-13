//! The lobby rules on their own: no HTTP, no Redis.

use miniarcade::{
    games::xo::{Mark, MoveError},
    lobby::{CODE_ALPHABET, Lobby, LobbyCode, LobbyError, LobbyStatus, Seat, Token},
};
use serde_json::json;

#[test]
fn codes_are_six_characters_from_the_alphabet() {
    for _ in 0..1000 {
        let code = LobbyCode::random();
        assert_eq!(code.as_str().len(), 6);
        assert!(
            code.as_str().bytes().all(|b| CODE_ALPHABET.contains(&b)),
            "{code:?}"
        );
    }
}

#[test]
fn the_alphabet_has_no_look_alike_characters() {
    for c in *b"0O1IL" {
        assert!(!CODE_ALPHABET.contains(&c), "{}", char::from(c));
    }
}

#[test]
fn parse_accepts_valid_codes_in_either_case() {
    assert_eq!(LobbyCode::parse("ABC234").unwrap().as_str(), "ABC234");
    assert_eq!(LobbyCode::parse("abc234").unwrap().as_str(), "ABC234");
}

#[test]
fn parse_rejects_everything_else() {
    let inputs = [
        "", "ABC23", "ABC2345", "ABC23O", "ABC231", "ABC23I", "ABC23L", "ABC 23", "AB/234",
        "ÄBC234",
    ];
    for input in inputs {
        assert_eq!(LobbyCode::parse(input), None, "{input:?}");
    }
}

#[test]
fn tokens_are_128_bit_hex_and_unique() {
    let (a, b) = (Token::random(), Token::random());
    assert_eq!(a.as_str().len(), 32);
    assert!(a.as_str().bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(a.as_str(), b.as_str());
}

#[test]
fn a_token_matches_only_itself() {
    let token = Token::random();
    assert!(token.matches(token.as_str()));
    let mut other = token.as_str().to_owned();
    let last = other.pop().unwrap();
    other.push(if last == '0' { '1' } else { '0' });
    assert!(!token.matches(&other));
    assert!(!token.matches(&token.as_str()[..31]));
    assert!(!token.matches(""));
}

#[test]
fn tokens_never_show_up_in_debug_output() {
    let token = Token::random();
    let lobby = Lobby::new(token.clone());
    assert!(!format!("{token:?} {lobby:?}").contains(token.as_str()));
}

fn playing_lobby() -> (Lobby, Token, Token) {
    let (one, two) = (Token::random(), Token::random());
    let mut lobby = Lobby::new(one.clone());
    lobby.join(two.clone()).expect("seat 2 is free");
    (lobby, one, two)
}

/// From a fresh game, the seat playing X takes the top row.
fn x_wins(lobby: &mut Lobby) {
    let x = lobby.view().x_seat;
    let o = x.other();
    for (seat, cell) in [(x, 0), (o, 3), (x, 1), (o, 4), (x, 2)] {
        lobby.play(seat, cell).unwrap();
    }
}

#[test]
fn a_new_lobby_waits_for_seat_two() {
    let view = Lobby::new(Token::random()).view();
    assert_eq!(view.status, LobbyStatus::Waiting);
    assert_eq!(view.turn, None);
    assert_eq!(view.x_seat, Seat::One);
    assert_eq!(view.version, 1);
    assert_eq!(view.board, [None; 9]);
}

#[test]
fn joining_fills_seat_two_and_starts_the_game() {
    let (lobby, _, _) = playing_lobby();
    let view = lobby.view();
    assert_eq!(
        (view.status, view.turn, view.version),
        (LobbyStatus::Playing, Some(Mark::X), 2)
    );
}

#[test]
fn a_full_lobby_turns_away_a_third_player() {
    let (mut lobby, _, _) = playing_lobby();
    assert_eq!(lobby.join(Token::random()), Err(LobbyError::LobbyFull));
    assert_eq!(lobby.version(), 2);
}

#[test]
fn tokens_identify_seats() {
    let (lobby, one, two) = playing_lobby();
    assert_eq!(lobby.seat_of(one.as_str()), Some(Seat::One));
    assert_eq!(lobby.seat_of(two.as_str()), Some(Seat::Two));
    assert_eq!(lobby.seat_of(Token::random().as_str()), None);
    assert_eq!(lobby.seat_of(""), None);
}

#[test]
fn seat_one_plays_x_and_moves_first() {
    let (mut lobby, _, _) = playing_lobby();
    assert_eq!(
        lobby.play(Seat::Two, 0),
        Err(LobbyError::Move(MoveError::NotYourTurn))
    );
    assert_eq!(lobby.play(Seat::One, 4), Ok(LobbyStatus::Playing));
    let view = lobby.view();
    assert_eq!(
        (view.board[4], view.turn, view.version),
        (Some(Mark::X), Some(Mark::O), 3)
    );
}

#[test]
fn nobody_moves_while_seat_two_is_empty() {
    let mut lobby = Lobby::new(Token::random());
    assert_eq!(
        lobby.play(Seat::One, 4),
        Err(LobbyError::WaitingForOpponent)
    );
    assert_eq!(lobby.version(), 1);
}

#[test]
fn refused_moves_keep_the_version() {
    let (mut lobby, _, _) = playing_lobby();
    lobby.play(Seat::One, 4).unwrap();
    assert_eq!(
        lobby.play(Seat::Two, 4),
        Err(LobbyError::Move(MoveError::CellTaken))
    );
    assert_eq!(lobby.version(), 3);
}

#[test]
fn a_win_ends_the_game() {
    let (mut lobby, _, _) = playing_lobby();
    x_wins(&mut lobby);
    let view = lobby.view();
    assert_eq!((view.status, view.turn), (LobbyStatus::XWon, None));
    assert_eq!(
        lobby.play(Seat::Two, 8),
        Err(LobbyError::Move(MoveError::GameOver))
    );
}

#[test]
fn rematch_needs_a_finished_game() {
    let (mut lobby, _, _) = playing_lobby();
    assert_eq!(lobby.rematch(), Err(LobbyError::GameNotOver));
    let mut waiting = Lobby::new(Token::random());
    assert_eq!(waiting.rematch(), Err(LobbyError::GameNotOver));
}

#[test]
fn rematch_swaps_x_and_o_and_clears_the_board() {
    let (mut lobby, _, _) = playing_lobby();
    x_wins(&mut lobby);
    let version = lobby.version();
    lobby.rematch().unwrap();
    let view = lobby.view();
    assert_eq!(view.x_seat, Seat::Two);
    assert_eq!(view.board, [None; 9]);
    assert_eq!(
        (view.status, view.turn, view.version),
        (LobbyStatus::Playing, Some(Mark::X), version + 1)
    );
    assert_eq!(
        lobby.play(Seat::One, 0),
        Err(LobbyError::Move(MoveError::NotYourTurn))
    );
    assert_eq!(lobby.play(Seat::Two, 0), Ok(LobbyStatus::Playing));
}

#[test]
fn a_lobby_survives_a_json_round_trip() {
    let (mut lobby, one, two) = playing_lobby();
    lobby.play(Seat::One, 4).unwrap();
    let stored: Lobby = serde_json::from_str(&serde_json::to_string(&lobby).unwrap()).unwrap();
    assert_eq!(stored.view(), lobby.view());
    assert_eq!(stored.seat_of(one.as_str()), Some(Seat::One));
    assert_eq!(stored.seat_of(two.as_str()), Some(Seat::Two));
}

#[test]
fn the_public_view_has_exactly_the_api_fields() {
    let (mut lobby, _, _) = playing_lobby();
    lobby.play(Seat::One, 4).unwrap();
    assert_eq!(
        serde_json::to_value(lobby.view()).unwrap(),
        json!({
            "board": [null, null, null, null, "X", null, null, null, null],
            "status": "playing",
            "turn": "O",
            "x_seat": 1,
            "version": 3,
        })
    );
}

#[test]
fn statuses_use_the_api_names() {
    let statuses = [
        LobbyStatus::Waiting,
        LobbyStatus::Playing,
        LobbyStatus::XWon,
        LobbyStatus::OWon,
        LobbyStatus::Draw,
    ];
    let names = ["waiting", "playing", "x_won", "o_won", "draw"];
    for (status, name) in statuses.into_iter().zip(names) {
        assert_eq!(serde_json::to_value(status).unwrap(), name);
        assert_eq!(status.as_str(), name);
    }
}
