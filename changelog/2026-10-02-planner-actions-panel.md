# The planner's action panel

- **Pages are tabs** along the top right (PLAN, HULLS, DESIGN, PLANS): the current one lit,
  TAB on the next one along.
- **Each page's actions are a panel of cells** along the bottom, like the flight instruments
  ("PLAN - ACTIONS"). Each cell has its key and a lamp:
  - dim when it can't do anything now: BUILD HERE away from a station or with a plan that won't
    go together; TURN IT on the commission row; COMMISSION with a design that won't go
    together; DROP IT on the keep row;
  - amber while armed: AGAIN TO BUILD.
- **The scattered key hints are gone:** the footer and the inline texts.
- **Fixed:** the module cursor kept its place from the last slot list. On a hull with fewer
  modules for a slot it pointed past the end, which read as "empty", so a sound plan said "WON'T
  GO TOGETHER - NO POWER". It's held within the list, and set afresh when a plan is loaded.
