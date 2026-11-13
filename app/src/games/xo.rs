//! XO (noughts and crosses) for two players on a 3×3 board.
//!
//! Cells are numbered 0 to 8, row by row from the top left:
//!
//! ```text
//! 0 | 1 | 2
//! 3 | 4 | 5
//! 6 | 7 | 8
//! ```
//!
//! X always moves first and the players alternate, so the board alone says
//! whose turn it is and how the game stands. [`Game`] therefore stores only
//! the board, and a stored game can't contradict itself.

/// A player's mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    X,
    O,
}

/// How a game stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Playing,
    XWon,
    OWon,
    Draw,
}

/// Why [`Game::play`] refused a move. A refused move leaves the game unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    /// The game already ended in a win or a draw.
    GameOver,
    /// The other mark moves next.
    NotYourTurn,
    /// The cell number is not 0 to 8.
    InvalidCell,
    /// The cell already holds a mark.
    CellTaken,
}

/// One game of XO.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Game {
    cells: [Option<Mark>; 9],
}

impl Game {
    /// An empty board with X to move.
    pub fn new() -> Self {
        Self::default()
    }

    /// The board, indexed by cell number.
    pub fn cells(&self) -> &[Option<Mark>; 9] {
        &self.cells
    }

    /// How the game stands.
    pub fn status(&self) -> Status {
        todo!()
    }

    /// Whose turn it is, or `None` once the game is over.
    pub fn turn(&self) -> Option<Mark> {
        todo!()
    }

    /// Puts `mark` on `cell` and returns how the game stands afterwards.
    pub fn play(&mut self, _mark: Mark, _cell: usize) -> Result<Status, MoveError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays `cells` in order, alternating from X, and returns the game.
    fn play_all(cells: &[usize]) -> Game {
        let mut game = Game::new();
        for &cell in cells {
            let mark = game.turn().expect("game still in play");
            game.play(mark, cell).expect("legal move");
        }
        game
    }

    /// Every winning line, built from the board's geometry rather than
    /// copied from the code under test.
    fn every_line() -> Vec<[usize; 3]> {
        let rows = (0..3).map(|r| [3 * r, 3 * r + 1, 3 * r + 2]);
        let columns = (0..3).map(|c| [c, c + 3, c + 6]);
        rows.chain(columns).chain([[0, 4, 8], [2, 4, 6]]).collect()
    }

    #[test]
    fn new_game_is_empty_and_x_moves_first() {
        let game = Game::new();
        assert_eq!(game.cells(), &[None; 9]);
        assert_eq!(game.status(), Status::Playing);
        assert_eq!(game.turn(), Some(Mark::X));
    }

    #[test]
    fn o_cannot_move_first() {
        let mut game = Game::new();
        assert_eq!(game.play(Mark::O, 4), Err(MoveError::NotYourTurn));
    }

    #[test]
    fn turns_alternate() {
        let mut game = Game::new();
        assert_eq!(game.play(Mark::X, 4), Ok(Status::Playing));
        assert_eq!(game.turn(), Some(Mark::O));
        assert_eq!(game.play(Mark::X, 0), Err(MoveError::NotYourTurn));
        assert_eq!(game.play(Mark::O, 0), Ok(Status::Playing));
        assert_eq!(game.turn(), Some(Mark::X));
    }

    #[test]
    fn a_move_puts_the_mark_on_the_cell() {
        let game = play_all(&[4, 0]);
        let mut expected = [None; 9];
        expected[4] = Some(Mark::X);
        expected[0] = Some(Mark::O);
        assert_eq!(game.cells(), &expected);
    }

    #[test]
    fn taken_cell_is_refused() {
        let mut game = play_all(&[4]);
        assert_eq!(game.play(Mark::O, 4), Err(MoveError::CellTaken));
    }

    #[test]
    fn cell_outside_the_board_is_invalid() {
        let mut game = Game::new();
        for cell in [9, 10, usize::MAX] {
            assert_eq!(game.play(Mark::X, cell), Err(MoveError::InvalidCell));
        }
    }

    #[test]
    fn x_wins_on_every_line() {
        for line in every_line() {
            // O takes two cells off the line, so only X can complete one.
            let others: Vec<usize> = (0..9).filter(|cell| !line.contains(cell)).collect();
            let mut game = play_all(&[line[0], others[0], line[1], others[1]]);
            assert_eq!(game.play(Mark::X, line[2]), Ok(Status::XWon), "{line:?}");
        }
    }

    #[test]
    fn o_can_win() {
        // X: 0, 1, 8. O: 3, 4, then 5 completes the middle row.
        let mut game = play_all(&[0, 3, 1, 4, 8]);
        assert_eq!(game.play(Mark::O, 5), Ok(Status::OWon));
    }

    #[test]
    fn full_board_without_a_line_is_a_draw() {
        // X O X
        // X O O
        // O X X
        let game = play_all(&[0, 1, 2, 4, 3, 5, 7, 6, 8]);
        assert_eq!(game.status(), Status::Draw);
    }

    #[test]
    fn completing_a_line_on_the_last_cell_is_a_win_not_a_draw() {
        // X O X
        // O X O
        // O X X   X's ninth move, cell 8, completes the 0-4-8 diagonal.
        let mut game = play_all(&[0, 1, 2, 3, 4, 5, 7, 6]);
        assert_eq!(game.play(Mark::X, 8), Ok(Status::XWon));
    }

    #[test]
    fn no_moves_after_a_win() {
        let mut game = play_all(&[0, 3, 1, 4, 2]);
        assert_eq!(game.status(), Status::XWon);
        assert_eq!(game.turn(), None);
        assert_eq!(game.play(Mark::O, 8), Err(MoveError::GameOver));
        assert_eq!(game.play(Mark::X, 8), Err(MoveError::GameOver));
    }

    #[test]
    fn no_moves_after_a_draw() {
        let mut game = play_all(&[0, 1, 2, 4, 3, 5, 7, 6, 8]);
        assert_eq!(game.turn(), None);
        assert_eq!(game.play(Mark::O, 0), Err(MoveError::GameOver));
    }

    #[test]
    fn refused_moves_leave_the_game_unchanged() {
        let mut game = play_all(&[4]);
        let before = game.clone();
        assert_eq!(game.play(Mark::X, 0), Err(MoveError::NotYourTurn));
        assert_eq!(game.play(Mark::O, 4), Err(MoveError::CellTaken));
        assert_eq!(game.play(Mark::O, 9), Err(MoveError::InvalidCell));
        assert_eq!(game, before);
    }

    #[derive(Debug, Default, PartialEq)]
    struct Totals {
        x_won: u32,
        o_won: u32,
        draw: u32,
    }

    /// Plays every legal continuation of `game` to the end.
    fn explore(game: &Game, totals: &mut Totals) {
        let mark = game.turn().expect("only games in play are explored");
        for (cell, slot) in game.cells().iter().enumerate() {
            if slot.is_some() {
                continue;
            }
            let mut next = game.clone();
            match next.play(mark, cell).expect("empty cell on your turn") {
                Status::Playing => explore(&next, totals),
                Status::XWon => totals.x_won += 1,
                Status::OWon => totals.o_won += 1,
                Status::Draw => totals.draw += 1,
            }
        }
    }

    #[test]
    fn every_possible_game_matches_the_known_totals() {
        // The well-known count when play stops at the first win: 255,168
        // games, of which X wins 131,184, O wins 77,904 and 46,080 are draws.
        let mut totals = Totals::default();
        explore(&Game::new(), &mut totals);
        let expected = Totals {
            x_won: 131_184,
            o_won: 77_904,
            draw: 46_080,
        };
        assert_eq!(totals, expected);
    }
}
