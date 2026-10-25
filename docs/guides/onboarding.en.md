# Access to your apps

You received two things, by different routes:

- a certificate file, `client.p12`
- its password

Keep both private. Anyone with both can enable access to the apps, so do not forward them. The administrator gives you the **Portal address**: `https://____________________`.

## 1. Install the certificate

### Android (Firefox or Chrome)

1. Save `client.p12` on the phone.
2. Open Settings and search for "certificate". Choose **Install a certificate**, then **VPN & app user certificate**. The menu names differ by vendor and Android version; a screen lock must be set.
3. Pick `client.p12` and enter the password.

### iPhone (Safari)

1. Open `client.p12` from Files, Mail or AirDrop. iOS says "Profile Downloaded".
2. Open Settings and tap **Profile Downloaded** at the top. If it is missing, go to General, VPN & Device Management.
3. Tap **Install**, enter the phone passcode and the certificate password.

Install it once per device.

## 2. Bookmark the Portal

Open the Portal address and add it to your bookmarks or home screen. When the browser asks which certificate to use, pick the one you installed.

## 3. Enable access

Open the bookmark and tap **Enable access**. The page shows "Available until" with a date. Your apps then work on this connection until that time, with their usual login.

- Tap **Extend access** to start the full period again.
- **Disable access for this connection** ends it early.
- A new connection needs it again, for example mobile data instead of Wi-Fi, another Wi-Fi, or a VPN. The Portal then says "Your connection changed."
- On your home network the Portal says "You're on your home network." Nothing to do.

## TV and other devices

A TV cannot install the certificate and does not need to. It uses the Wi-Fi's connection, so enable access from a phone on the same Wi-Fi as the TV, then open the app on the TV.

## If something does not work

| What you see                                                               | Meaning                                                  | Do                                                                                          |
| -------------------------------------------------------------------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| An app shows **Forbidden** (403)                                           | Access is not enabled for this connection, or it expired | Open the Portal and tap **Enable access**                                                   |
| **Forbidden** right after enabling                                         | The apps may see another address than the Portal         | Open **Troubleshooting** in the Portal and send the Address and Family to the administrator |
| The Portal does not open, or the browser shows an SSL or certificate error | The certificate is missing, wrong or expired             | Install it again; ask the administrator for a new file                                      |
| The Portal keeps loading in Chrome on Android                              | Chrome may not be offering the certificate               | Force-stop Chrome in the Android settings and try again, or use Firefox                     |
| "We couldn't enable access."                                               | The service is unavailable                               | Tap **Try again**; if it persists, tell the administrator                                   |
| "Please wait a moment before trying again."                                | Too many attempts                                        | Wait for the countdown                                                                      |
