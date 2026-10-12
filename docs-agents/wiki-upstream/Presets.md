SRS supports creation presets, mainly targetting at the overlays (either EAM or radio).

There are two main kinds:

* `awacs.json`: This gives you full control on the AWACS panel, allowing you to precisely configure which radio does what.
* Channelization text file (`UHF Guard.txt`): A list of name/frequency pairs to more easily identify frequencies in the channel list.

## Channelization file

Fixed channels can be created for all FC3 aircraft and all expansion radios for Non FC3.

Channel presets are created per Radio - full list of names & frequencies here OR just look at the SRS overlay and copy the name of the radio from there 🙂 https://docs.google.com/spreadsheets/d/1tzd996zJ1t0heZ-t1PpL7vNUIZbXl7pI6De0GThN1Qw/edit#gid=1869108790 ) 

Preset files are named after the radio (no special characters needed & case is ignored) and are text files with a single frequency on each line.

For example, to create a fixed channel list for the first UHF radio in the F15C:

Create a file called `ANARC-164 UHF1.txt` or `anarc164 UHF1.txt` where you installed SimpleRadio Standalone (where the client exe is), add frequencies and save. 

The file will be automatically loaded when that radio appears on an aircraft. The frequency list can also be manually reloaded with the reload button

You can also name channels like so in your text file:
```
TACOM|267.75
AWACS|251.0
```


## Customise Radios for EAM

SRS supports the ability for you to customise the radios that appear in EAM. You can configure:

- Name

  The name of the radio, that will show in the Overlay.
  It's also used as the base name for the txt channelization file.

- Modulation

  0 = AM, 1 = FM, 2 = INTERCOM, 3 = DISABLED, 4 = HAVEQUICK, 5 = SATCOM, 6 = MIDS, 7 = SINCGARS

- Encryption
- Guard (secondary frequency)
- Max and Min frequencies

To do this - firstly go to where ever you installed SRS - by default in program files and copy the `awacs-radios.json` file - renaming the copy `awacs-radios-custom.json`.

This is the safest way to make changes as the **awacs-radios.json will be overwritten with every SRS update**

To create a MIDS radio - edit any radio other than the first to match the below - save the file and then connect using EAM. 

```json
{
    "enc": false,
    "encKey": 1,
    "encMode": 1,
    "freqMax": 1060000000.0,
    "freqMin": 1030000000.0,
    "freq": 1030100000,
    "modulation": 6,
    "name": "MIDS Radio",
    "secFreq": 0.0,
    "volume": 1.0,
    "freqMode": 1,
    "volMode": 1,
    "expansion": false,
    "channel": -1,
    "simul": false,
    "rtMode": 1
  },
```

The magic there is the modulation of type 6 which sets it to MIDS

You can also use type 0 for AM and type 1 for FM modulation

You'll see the radio has all 126 MIDS radio channels selectable in the radio channel dropdown

## Server-side Preset

as of 2.2.0.4, servers can now "push" presets to the connected clients, allowing shared named channelization.

Create a `Presets/` folder next to the server configuration file used (server.cfg by default, or the one passed in with `-cfg=`).
Add preset channelization as described in the [Channelization file section](#channelization-file).

NOTE: you can find all the radio names by looking at the `DCS-SimpleRadioStandalone.lua` that ships in client scripts. The names are normalized alphanumerical lowercaser (`AN/ARC-210 COMM2` becaomes `anarc210comm2`). 

In the server configuration, enable `Server Channel Presets`.

If clients have opted in to the server channel presets, their channel list will reflect the server's.