# Security

Vaexil Mod Manager runs with your normal user rights. It never asks for admin rights. Its only network connections are to GitHub, over HTTPS, to check for and download SMF and Vaexil Mod Manager updates. The [safety model](docs/safety-model.md) lists every file it reads and writes. In short:

- It never changes Simple Mod Framework's own code or the files SMF ships, and it sends nothing to SMF's authors.
- Inside SMF's folder it writes only `config.json` (the load order and mod options, with a backup first), sets aside a stale `cleanThumbs.dat` before a deploy, and replaces SMF's program files when you choose to update SMF. Inside HITMAN's folder it writes nothing itself; a deploy is SMF's own `Deploy.exe` at work.
- Its own files (settings, setups, logs, plans, the change journal and backups) go in `%LOCALAPPDATA%\Vaexil Mod Manager`. Only when you confirm it does it change your PATH, HITMAN's launch options in Steam, or an AI app's MCP settings. Every new write is listed in the safety model before it ships.

## Reporting a problem

Report anything that could harm a user's game, mods or PC privately, through GitHub: **Security > Report a vulnerability** on this repository. Please do not open a public issue for these.

Examples worth reporting:

- the app writing anywhere the safety model says it does not, especially inside HITMAN's or SMF's folder;
- the app changing, replacing or rebuilding any of SMF's own files, such as `Deploy.exe`;
- a crafted `manifest.json`, SMF `config.json`, Steam or Epic file that makes the app run a program or read files outside what the safety model lists.

Include the app version (the bottom of the window shows it), what you did, and what happened.

If the problem is in Simple Mod Framework itself, for example a deploy that fails the same way in SMF's own Mod Manager, report it to [SMF's authors](https://github.com/atampy25/simple-mod-framework) instead.

You should get a reply within a week. Fixes go into the next release, and the report is credited unless you would rather it was not.

## Supported versions

Only the latest release gets fixes.
