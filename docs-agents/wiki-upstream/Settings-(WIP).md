![Settings screen of SRS](https://github.com/ciribob/DCS-SimpleRadioStandalone/raw/master/docs/SRS_Settings_screen.png)  

# Contents
* [Global](#global-settings-wip)
* [Profile](#profile)
* [Profile settings](#profile-settings-wip)


## Global settings (WIP)
#### Auto Connect
* When joining a DCS multiplayer server, allow SRS automatically connect to the
 SRS server provided by DCS server.

#### Auto Connect Prompt
* Instead of automatically connecting, prompt the user to connect to the
 provided server (see [Auto Connect](#auto-connect)).

#### Auto Connect Mismatch Prompt
* 

#### Reset Radio Overlay
* Reset radio overlay position and changes to defaults.

#### Hide Overlay Taskbar Item
* Remove the overlay window from the windows taskbar (unifies overlay into the
 main SRS taskbar entry)

#### Auto Refocus DCS
* Bring the DCS window back into focus, as the current application, after
 manipulating the radio overlay. Around a 2 second delay after moving the
 mouse off of the overlay.

#### Allow More Input Devices
* Allow almost any DirectX device to be used as an input, except a mouse, but
 may cause issues with other device detection.

#### Microphone Automatic Gain Control
* Allow SRS to manipulate microphone gain, to provide a more stable level of
 input.

#### Microphone Noise Supression
* Allow SRS to dampen input noise from the microphone.

#### Minimize to tray
* When minimized, minimize the SRS main window to the taskbar tray.

#### Start minimized
* Start with main window minimized to the taskbar tray.

#### Auto Select Profile For Aircraft
* Automatically switch to an SRS profile for any given aircraft, if an
 identically named profile exists; symbols and spaces excluded. For
 example, "av8b" would be automatically chosen when you select an AV-8B
 role. Must use full DCS scoreboard name.

#### Check for beta updates
* Allow SRS to check for updates, including those in the beta branch.

#### Play connection sounds
* Play sounds when connecting to an SRS server.

#### Set SRS Path for DCS
* Choose the DCS install path to be used with SRS.

#### Require Admin
* Run the application as Administrator. Required to intercept inputs mapped
 to both SRS and DCS, while still allowing DCS to process and use those
 inputs.

#### Allow VAICOM TX Inhibit (WIP)
* 

#### Show Transmitter Name
* Show the username of whoever is transmitting on the current frequencies,
 in the frequency windows on the overlays, as well as in the DCS in-game
 overlay (if enabled).

* Requires server "Show transmitter name" setting to be enabled.

#### Allow Anonymous Usage Stats
* Allow SRS to collect usage statistics.


## Profile
Where the current profile is selected, deleted, renamed, or cloned, as
 well as where new profiles are created.

## Profile settings (WIP)
### Controls/Cockpit Integration (WIP)
#### Radio Switch works as Push-To-Talk (PTT)
* Determine whether the "Radio 1", "Radio 2", etc. button will select that
 particular radio, or select _and transmit_ on that radio. Practically,
 this determines whether a separate PTT button is required (for radios
 with a button bound).

#### Auto Select First Channel Preset
* 

#### ALWAYS allow SRS Hotkeys
* 

#### ALWAYS allow Transponder Overlay Controls
* Determine whether the frequency control buttons always display on the
 radio overlay, or whether they only display when the current aircraft
 lacks radio controls.

#### Allow INCOCKPIT DCS Controlled PTT
* Determine whether (certain) DCS aircraft can control the SRS PTT with
 the in-game radio key transmit/selection bindings.

#### Rotarty Style Frequency Increment/Decrement
* When adjusting frequency with SRS hotkeys, allow values to roll over rather than increment. 
* Default behaviour: ```123.900 --> 124.000```
    Rotary Behaviour: ```123.900 --> 123.000```


#### Push-to-Talk Release Delay (ms) - SRS Hotkeys only
* Delay the end of transmission after the PTT key is released.

#### Push-to-Talk Start Delay (ms) - SRS Hotkeys only
* Delay the start of transmission after the PTT key is pressed.

---
### Radio Effect Settings
#### Radio Rx Effects
* Determines which of the "Start" and "End" transmission effects SRS will
 play when starting and ending a received transmission. Affects encryption
 and MIDS effects.

#### Radio Tx Effects
* Determines which of the "Start" and "End" transmission effects SRS will
 play when pressing and releasing the PTT button, respectively. Affects
 encryption and MIDS effects.

#### Radio Transmit Start Effect
* The effect played when "Start" is **ON** for
 [Radio Tx Effects](#radio-tx-effects) or [Radio Rx Effects](#radio-rx-effects)

#### Radio Transmit End Effect
* The effect played when "End" is **ON** for
 [Radio Tx Effects](#radio-tx-effects) or [Radio Rx Effects](#radio-rx-effects)

#### Radio Encryption Effects
* Play additional sounds when transmitting or receiving encrypted communications.

#### MIDS Tx & Rx effects
* Play additional sounds when transmitting or receiving MIDS communications.

#### Enable Radio Voice Effect
* Determines whether or not SRS will filter voices to sound like they do on actual
 radios.

#### Enable Clipping Effect
* Determines whether SRS will apply the clipping effect to loud transmissions.

#### Enable FM Radio Tone
* Determines whether SRS will play an additional tone when using FM radios.
* Slider allows for volume adjustment of this tone.

#### Enable HAVEQUICK Effect
* Determines whether SRS will apply an additional effect to radios using
 HAVE QUICK (HQ) mode.

#### Enable Background Radio Effects (UHF/VHF/HF/FM)
* Determines whether SRS will add background noise to radio transmissions.
* Sliders allow for volume adjustment on each of the 4 radio types.
 

---
### Audio
Sliders controlling which audio channel(s) a particular radio is played
 on (left or right), and the degree to which it is played on those
 channels (volume on each).