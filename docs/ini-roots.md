# INI root profiles

I adapted configurable root-file selection from my ra2-mono
`GameConfig.cs` / `GameIniLoader.cs` into VERA's existing source owners.
This selects one Rules, fixed Art and standalone AI root as a unit. It does
not replace the parser, ordered Rules processing, AI registry/map passes
or simulation authority.

## Configuration

Put these `[ini]` settings in `config.toml`. VERA checks the working directory
first, then the directory beside its executable. Relative paths in that config
resolve from the config's own directory; the INI basenames below resolve through
the selected game folder's resource provider.

```toml
[ini]
profile = "mo"
```

| Profile | Rules | Fixed Art | Standalone AI |
| --- | --- | --- | --- |
| `yr` (default) | `rulesmd.ini` | `artmd.ini` | `aimd.ini` |
| `mo` | `rulesmo.ini` | `artmo.ini` | `aimo.ini` |
| `auto` | One complete unambiguous preset triplet | Same preset | Same preset |
| `custom` | Explicit `rules` | Explicit `art` | Explicit `ai` |

```toml
[ini]
profile = "custom"
rules = "my-rules.ini"
art = "my-art.ini"
ai = "my-ai.ini"
```

Custom mode requires all three logical INI basenames. Paths and wildcards are
rejected; these names are resolved through the existing VFS, not arbitrary host
file access. Overrides cannot be mixed with a preset. Explicit `mo` and `custom`
require all three nonempty readable winners before consumers build data, and
never fall back to YR when a selected root is missing. `yr` preserves legacy
admission: individual consumers still report unavailable files at their original
stage, without eagerly requiring gameplay INIs in asset-only tools.

`auto` probes only files visible to the assembled resource provider. It accepts
YR 3/3 with MO 0/3, or MO 3/3 with YR 0/3. Both complete sets, a partial alternate
set, or no complete set are errors with counts and an explicit-selection hint.
This conservative policy avoids guessing from a directory name or silently
combining families. It detects root-file signatures, not a MOD release/version.

The asset manager freezes the resolved names before Rules/Art/AI consumers run.
Later theater or shell registrations do not rerun Auto. The process asset owner
retains that choice across a lost-manager reconstruction; it does not silently
change roots underneath an already constructed Rules registry. This is resource
configuration, not new gameplay state or a save-layout field.
Process-source diagnostics retain the selected logical Rules/Art filenames with
their frozen content hashes, rather than labelling MO/custom snapshots as
`RULESMD.INI` / `ARTMD.INI`. They do not reopen sources after scenario rebuilds.

## Inspection

```text
asset ini-roots --ra2-dir <game directory> --ini-profile yr
asset ini-roots --ra2-dir <game directory> --ini-profile auto
asset ini-roots --ra2-dir <game directory> --ini-profile custom --rules-ini r.ini --art-ini a.ini --ai-ini i.ini
asset ini-get TEST Voxel --domain art --reader bool --default false --ra2-dir <game directory> --ini-profile mo
```

INI commands use `[ini]` from the normal host config unless an explicit
`--ini-profile` request replaces the entire INI configuration. CLI custom mode
must supply its own complete triplet, not inherit missing fields from a config.
Other asset commands retain the retail setup and reject profile flags.
`ini-roots` reports the concrete choice and physical source identities through
the production selector. `ini-get` and `ini-type` report the same choice and read
the chosen Rules/fixed Art sources. Source hashes identify the bytes consumed,
not a claim of MOD gameplay support.

`ini-roots --all-mixes` explicitly uses the existing tool-only wider search.
It does not alter normal game mounting or bypass missing required retail media.
The GPU-free headless API also accepts an explicit root request through
`headless_scenario::load_with_launch_and_ini`; existing headless entry points
remain YR and never reread an unrelated cwd configuration.

## Mental Omega names

MO runs gamemd through its spawner (`cncnet5mo.dll`), which rewrites gamemd's
file names before startup. An explicit `mo` profile applies the same renames:

| YR name | MO name |
| --- | --- |
| `expandmd##.mix` (99..00) | `expandmo##.mix` |
| `mapsmd##.mix`, `multimd.mix` | `mapsmo##.mix`, `multimo.mix` |
| `movmd##.mix`, `thememd.mix` | `movmo##.mix`, `thememo.mix` |
| `soundmd.ini`, `evamd.ini`, `thememd.ini` | `soundmo.ini`, `evamo.ini`, `thememo.ini` |

MO's own Ares `debug.log` confirms the archive order (EXPANDMO99..94, mapsmo03,
MULTIMO, movmo03, no EXPANDMD probe). Master, cache, local, audio, conquer and
cameo archives keep their names. Archive names follow only an explicit `mo`:
`auto` probes roots after mounting, so it keeps the YR archives. The asset
tool's INI commands mount the same family; other asset commands stay YR.

## Scope and source

Mode/campaign catalogs, the spawner's BATTLEMO/MAPSELMO/MISSIONMO and mode INI
renames, UI, localization, arbitrary MOD archives and Ares/Phobos gameplay tags
are not retargeted. MO's mode INIs (`MPBattleMO.ini` etc.) are absent from its
archives; its client supplies those settings. A directory missing required
archives still cannot enter the normal game bootstrap. Selecting the MO triplet does not certify that every MO
definition, asset or gameplay mechanism is supported.

[Include and inheritance](ini-directives.md) expand at the selected source
boundary only in gameplay builds with `--features ares`. Root selection itself
needs no extension feature. The read-only `ini-expand --source-only` probe works
in every build and does not require a configured Rules/Art/AI triplet.

The mono reference is my local working tree: configurable file lists in
`Ra2.Shared/GameConfig.cs`, consumed together by the cached
`Ra2.IO/GameIniLoader.cs` bundle and `Ra2.IO/IniMerger.cs`. VERA intentionally
retains one root per family rather than copying mono's multi-file merge or AI
whole-definition replacement policy. Conservative Auto detection and frozen
process choice are VERA adaptations, not an executable Ares/Phobos parity claim.
I contribute this adaptation under VERA's GPLv3.
