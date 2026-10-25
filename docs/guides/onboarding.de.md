# Zugriff auf deine Apps

Du hast zwei Dinge auf getrennten Wegen bekommen:

- eine Zertifikatsdatei, `client.p12`
- das zugehörige Passwort

Halte beides privat. Wer beides hat, kann den Zugriff auf die Apps aktivieren, also leite sie nicht weiter. Die **Portal-Adresse** bekommst du vom Administrator: `https://____________________`.

## 1. Zertifikat installieren

### Android (Firefox oder Chrome)

1. Speichere `client.p12` auf dem Telefon.
2. Öffne die Einstellungen und suche nach „Zertifikat“. Wähle **Zertifikat installieren**, dann **VPN- & App-Nutzerzertifikat**. Die Menünamen unterscheiden sich je nach Hersteller und Android-Version; eine Bildschirmsperre muss eingerichtet sein.
3. Wähle `client.p12` und gib das Passwort ein.

### iPhone (Safari)

1. Öffne `client.p12` in Dateien, Mail oder per AirDrop. iOS meldet „Profil geladen“.
2. Öffne die Einstellungen und tippe oben auf **Profil geladen**. Fehlt der Eintrag, gehe zu Allgemein, VPN & Geräteverwaltung.
3. Tippe auf **Installieren** und gib den Code des Telefons und das Zertifikatspasswort ein.

Die Installation ist pro Gerät einmal nötig.

## 2. Portal als Lesezeichen speichern

Öffne die Portal-Adresse und speichere sie als Lesezeichen oder auf dem Home-Bildschirm. Fragt der Browser, welches Zertifikat er verwenden soll, wähle das installierte.

## 3. Zugriff aktivieren

Öffne das Lesezeichen und tippe auf **Zugriff aktivieren**. Die Seite zeigt „Gültig bis“ mit einem Datum. Bis dahin funktionieren deine Apps auf dieser Verbindung, mit ihrem gewohnten Login.

- **Zugriff verlängern** startet den vollen Zeitraum neu.
- **Zugriff für diese Verbindung deaktivieren** beendet ihn vorzeitig.
- Eine neue Verbindung braucht die Aktivierung erneut, zum Beispiel mobile Daten statt WLAN, ein anderes WLAN oder ein VPN. Das Portal meldet dann „Deine Verbindung hat sich geändert.“
- In deinem Heimnetz meldet das Portal „Du bist in deinem Heimnetz.“ Dann ist nichts zu tun.

## Fernseher und andere Geräte

Ein Fernseher kann das Zertifikat nicht installieren und braucht es auch nicht. Er nutzt die Verbindung des WLANs. Aktiviere den Zugriff daher mit einem Telefon im selben WLAN wie der Fernseher und öffne danach die App am Fernseher.

## Wenn etwas nicht funktioniert

| Was du siehst                                                                 | Bedeutung                                                        | Tu das                                                                                                      |
| ----------------------------------------------------------------------------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Eine App zeigt **Forbidden** (403)                                            | Der Zugriff ist für diese Verbindung nicht aktiv oder abgelaufen | Öffne das Portal und tippe auf **Zugriff aktivieren**                                                       |
| **Forbidden** direkt nach der Aktivierung                                     | Die Apps sehen möglicherweise eine andere Adresse als das Portal | Öffne **Fehlerbehebung** im Portal und sende Adresse und Adressfamilie an den Administrator                 |
| Das Portal öffnet nicht, oder der Browser meldet einen SSL-/Zertifikatsfehler | Das Zertifikat fehlt, ist falsch oder abgelaufen                 | Installiere es erneut; frage den Administrator nach einer neuen Datei                                       |
| Das Portal lädt in Chrome auf Android endlos                                  | Chrome bietet das Zertifikat womöglich nicht an                  | Beende Chrome in den Android-Einstellungen („Beenden erzwingen“) und versuche es erneut, oder nutze Firefox |
| „Wir konnten den Zugriff nicht aktivieren.“                                   | Der Dienst ist nicht erreichbar                                  | Tippe auf **Erneut versuchen**; bleibt es so, sage dem Administrator Bescheid                               |
| „Bitte warte einen Moment, bevor du es erneut versuchst.“                     | Zu viele Versuche                                                | Warte den Countdown ab                                                                                      |
