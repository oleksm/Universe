# Commerce in three files (audit phase 5)

- `sim/commerce.rs` (814 lines) is a directory: `commerce/mod.rs` (price boards over the hypernet,
  pilots' trades, quotes, the trade log), `yard.rs` (refits, hulls bought, repairs, the vending
  machine, trim) and `passengers.rs` (passage booked and landed, the fares). Nothing renamed:
  `commerce::Booking` and `commerce::BUYBACK` are where they were.
- The engine-boundary test (`tests/boundary.rs`) reads a module that's a directory, every file
  in it (checked: a cockpit reference put in `commerce/yard.rs` fails it).
