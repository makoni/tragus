# Tragus distribution data

Files installed alongside the binary:

| File | Destination |
| --- | --- |
| `me.spaceinbox.tragus.desktop` | `$datadir/applications/` |
| `me.spaceinbox.tragus.metainfo.xml` | `$datadir/metainfo/` |
| `icons/hicolor/scalable/apps/me.spaceinbox.tragus.svg` | `$datadir/icons/hicolor/scalable/apps/` |
| `icons/hicolor/symbolic/apps/me.spaceinbox.tragus-symbolic.svg` | `$datadir/icons/hicolor/symbolic/apps/` |

Sanity checks before publishing:

```sh
desktop-file-validate data/me.spaceinbox.tragus.desktop
appstreamcli validate data/me.spaceinbox.tragus.metainfo.xml
```

The icons here are placeholder geometry — a stylised ear in GNOME blue.
A proper icon needs designing before Flathub submission.
