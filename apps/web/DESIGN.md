# Portal design

One screen, phone first. A Credential holder opens it, reads one status line and taps one button. Copy and states: `docs/spec.md` section 9.

## Rules

- **One primary action per state**, full width on phones, at least 44 px tall. Revoke is secondary and asks for confirmation.
- **Status in words first.** Colour and icons support the text; they never carry the meaning alone.
- **No infrastructure words** on the main screen: no IP, certificate, host, policy, duration. The Address and its family live in the troubleshooting disclosure.
- **Announce changes**: the status line is a polite live region; errors are assertive.
- **320 px minimum width**, no horizontal scroll, visible focus on every control, full keyboard use.
- **Reduced motion**: transitions only through `motion-safe:`; nothing moves for users who opted out.
- **Light and dark** follow the OS (`prefers-color-scheme`); tokens in `src/app.css`. Keep the contrast budget noted there.
- **No external requests**: system fonts, no CDNs, no analytics, no service worker.
- **Budget**: initial HTML, CSS and JS under 150 KiB compressed.

## Components

Compose from `#lib/components/ui` (shadcn-svelte: `button`, `dialog`). Add another shadcn component only when the screen needs it, and check the budget afterwards.
