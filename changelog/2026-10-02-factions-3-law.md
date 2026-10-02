# Factions 3: faction law and security

- **Each faction's law** (`factions.ron`): `aggression`, how long one who opens fire on the
  innocent stays fair game in its space (Concord 15 min, Reach 5, Directorate 10), and `hostile`,
  the standing at or below which it treats a pilot as an enemy (-40, -70, -50).
- **Unclaimed space has no law:** the law service rules by the holder of the space the struck ship
  is in; with no holder, no ruling (`Law::hit` takes the law's duration, or none).
- **Security:** the holder's turrets fire on the fair game and on its enemies (a ship's snapshot
  carries `hostile`, by its standing with the holder; gunners fire on `wanted`: either); its docks
  refuse its enemies: "REFUSED - HALDEN CONCORD TREATS YOU AS AN ENEMY".
- **A loss pays the debt:** a ship that comes back after being lost is no longer an enemy of the
  holder where it returns (just above the line: no friend). No respawning into the guns.
- **HUD:** "ENEMY OF THE HALDEN CONCORD - ITS GUNS FIRE, ITS DOCKS REFUSE"; STAND turns red.
  Warnings now start under the status strip; the top bar takes seven buttons a row.
- The turret test now also makes the innocent an enemy by standing: refused docking, shot down.
  Dev scenario `enemy`.
