// Phone layout (round 15): narrow screens get one column with the views on a bar along the
// bottom (pages/PhoneShell). Decided by width, so a phone turned sideways or a tablet with
// room for it gets the computer layout.

const QUERY = '(max-width: 600px)';

export const phone = $state({ on: false });

if (typeof window !== 'undefined' && window.matchMedia) {
  const list = window.matchMedia(QUERY);
  phone.on = list.matches;
  list.addEventListener('change', (e) => (phone.on = e.matches));
}
