// The Portal is one prerendered shell; the binary embeds and serves it. Everything
// on it depends on the requesting connection, so it renders in the browser only.
export const prerender = true;

export const ssr = false;
