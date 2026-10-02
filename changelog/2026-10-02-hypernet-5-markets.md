# Hypernet 5: markets on knowledge

- **Price boards:** every market in the gate network puts out its board (a quote for every good)
  every 2 minutes. Another market has a board once it's come over the hypernet: the board's
  market's lag from the backbone, and its own. A market off the net hears nothing new and isn't
  heard. The world's first boards are long known everywhere (it's been trading for ages).
- **Traders decide on what their market knows:** the quotes from elsewhere in a market's answer
  are the boards that have reached it, with their age. A port 40 light-minutes out is known as it
  was then; the trip there may find otherwise.
- **The market screen is no longer all-seeing:** live where you're docked or landed; any other
  market shows its board as it reached you ("ITS BOARD AS IT REACHED US OVER THE HYPERNET: 76 S
  OLD"), its long-known prices, or "NO WORD OF ITS PRICES REACHES US".
- Dev scenarios `marketnear`, `marketfar`. The trader test checks a station's prices seen from
  away are a board, not live.
