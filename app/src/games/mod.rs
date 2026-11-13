//! Game rules, one module per game. A rules module is pure (no I/O, clock or
//! randomness), so it is tested on its own and adding a game only adds a
//! module next to `xo`.

pub mod xo;
