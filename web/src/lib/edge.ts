/**
 * Remembers a yes/no value and says when it flips: 'on' the moment it turns true, 'off' the
 * moment it turns false, null while it stays as it was.
 *
 * For effects that must do something once when a thing opens. An effect re-runs whenever
 * anything it read changes, so "is it open?" alone would redo the opening work every time
 * (round 21: Find wiped its box on every keystroke that way).
 */
export function edge(initial = false): (now: boolean) => 'on' | 'off' | null {
  let was = initial;
  return (now) => {
    if (now === was) return null;
    was = now;
    return now ? 'on' : 'off';
  };
}
