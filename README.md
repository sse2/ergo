# ergo

a macos utility that monitors display connections and runs commands based on configurable rules.

## build

requires rust (2024 edition) and both x86_64 and arm64 toolchains, if you want to use the `./build/package.sh` script.

```sh
./build/package.sh
```

alternatively, you can run `cargo run -r`. be wary that if you don't have the binary packaged in a macos .app the app will freeze when trying to show the first run dialog. this can be bypassed by simply having any config.

## config

ergo reads from `$XDG_CONFIG_HOME/ergorc`, defaulting to `~/.config/ergorc`. after running the app and getting past the first-run dialog, if a config isn't present, one will be generated for you.

### config options

```
verbose          # enable debug logging
firstrun         # re-show first-run dialog
yesservice       # install as launchagent on startup
noservice        # remove launchagent if installed
```

### config rules

rules follow the format `<conditions> -> <command>`. commands are executed with `$SHELL` (defaults to `/bin/zsh` if not present).

**conditions:**

---------------------------------------------
|  syntax  |            meaning             |
|----------|--------------------------------|
| `+`      | display added                  |
| `-`      | display removed                |
| `"name"` | display with name is connected |
| `=N`     | exactly N displays connected   |
| `>N`     | more than N displays           |
| `<N`     | fewer than N displays          |
---------------------------------------------

conditions can be combined with `and` / `or`. conditions are evaluated **from left to right**, with no operator precedence (first come first serve).
for example:
```
the input: 
+ and "G27QC A" and =1 -> echo 'hello'

evaluates to the following condition tree:
      and
     /   \
    +    and
        /    \
    "G27QC A"  =1
```

**examples:**

```
verbose
yesservice
+ and "Built-in" and =2 -> echo 'external display connected'
"G27QC A" -> open /Applications/DisplayCenter.app
- and =1 -> echo 'back to single display'
```

## service

ergo can install itself as a launchagent (`cat.fennec.ergo`) for automatic startup. use the `yesservice` / `noservice` config options or the first-run dialog.

logs are written to `/tmp/ergo-log-<username>.log`.
