# Welcome to the DCS-SimpleRadio Stand Alone wiki!

If you find any information in this wiki/documentation which requires correction or updating, please leave a comment, contact one of the contributors, or leave a message on the [SRS discord](https://discord.com/channels/298054423656005632/473947464349646848).

# Contents
* [Features & Project Goals](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki#features--project-goal)
* [Installation/Updating](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki#installation--setup)
* [How to Uninstall](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki#how-to-uninstall)
* [Client Setup](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki#client-setup)
* [SimpleRadio in DCS](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki#simpleradio-in-dcs)
* [Encryption](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki#encryption)
* [SRS Server Setup](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki/SRS-Server-(WIP))
* [Roadmap/Wishlist](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki#roadmapwishlist)
* [Troubleshooting](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki#troubleshooting)
* [Acknowledgements & Thanks](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki#acknowledgements--thanks)


## Features & Project Goal
The DCS SimpleRadio Standalone (DCS-SRS) is a project originating as a TeamSpeak 3 plugin called DCS-SimpleRadio.

The goal of this project is to bring integrated Radio Comms into DCS similar to my existing TS plugin and other plugins (Aries, UniversRadio, TARS), however, **without** relying on TeamSpeak. 

SimpleRadio Standalone is meant to complement other VOIP software, such as TeamSpeak or Discord, allowing players to use "full radio voice comms" in DCS World, should the player desire, along with the option for the SRS client to automatically connect to a DCS-SRS multiplayer server. TS/Discord is then used as a fall back to establish frequencies, or for those that don't have the radio.

As of September 26th, 2016, the following features are available:

* All FC3 Aircraft with Realistic Radios and Frequencies + Guard Frequencies.
* All "Full Fidelity Clickable" cockpit aircraft including the Gazelle.
* Hotkeys for PTT + Switching Active Radio (Gazelle + A10C + FC3 only)*.
* In Cockpit PTT (UH-1H, Mi-8, L-39, F-86, MiG-15, MiG-19, Mig-21, F-5E, P-51D/TF-51D, Yak-52, F-14)*.
* Intercom Support in the L-39, UH-1H, SA342 Gazelle, *.
* GCI / CA / ATC Radio
* Voice encryption for A-10C / Mirage / A-10A / F-15C / Gazelle / F/A-18C
* Radio Overlay indicating current frequencies, selected radio and active light when Transmitting or Receiving
* Built from Scratch voice protocol using OPUS encoding - allows much greater flexibility as I'm not dependent on TS3 or Mumble.
* Client Microphone Boost
* Audio Separation - Left, Right or Both
* Radio Effects for voice
* Optional Line of Sight limitations (Server side)
* Optional Radio Distance Limitations (Server Side)
* Signal fading (volume) for Line of Sight Loss and over Distance
* Expansion Radios - Ability to expand the existing radio set on the aircraft for training and ease of communication. (Server Side)
* Optional IRL Transmit behaviour - Stops you hearing transmissions when transmitting on a radio (Server Side)

_*likely requires updating and/or corrections_

## Installation & Setup

### Using the Auto-Updater (preferred)

Download and run the [latest SRS-AutoUpdater.exe](https://github.com/ciribob/DCS-SimpleRadioStandalone/releases/latest/download/SRS-AutoUpdater.exe).

Follow the instructions on screen, read carefully.

<details>
<summary><h3>Manually using the installer (backup method)</h3></summary>
1. Download and **extract** the .zip containing the latest release - available here: https://github.com/ciribob/DCS-SimpleRadioStandalone/releases

2. Run the Installer.exe
NOTE: If Windows prevents the .exe from running, click "Show More" followed by "Run Anyway"; this applies to the Client and Server executables as well.

3. Step 1: Pick Install Location - This is the arbitrary install path for SimpleRadio Standalone, typically something like C:\Program Files\DCS-SimpleRadio-Standalone
Note this path, so you are able to navigate to the program folder after installation

4. Step 2: Locate Saved Games - Locate the folder that DCS settings are saved to, typically C:\Users\CurrentUser\Saved Games\.

5. Step 3: Install Or Remove - To install or update to the latest version of DCS-SimpleRadio Standalone, click the 'Install/Update SimpleRadio' button. The installer may hang or become unresponsive until the dialogue indicating installation has completed appears. Updating should not change your keybindings! (We're thinking of you <3)

6. All done! The folder you selected for 'Step 1: Pick Install Location' should now open. If you would like, create a shortcut for the 'SR-ClientRadio.exe' and/or 'SR_Server.exe' depending on which you would like to  use.
</details>

## How to Uninstall
Run the 'Installer.exe' and click the 'Remove DCS-SimpleRadio Standalone' button; If you have any feedback that would have prevented the uninstallation, please contact the project owner on GitHub!

## Client Setup
First, you must run the 'SR-ClientRadio.exe' in order to configure (and connect to servers of) SimpleRadio!
#### General
Here you can select your Input (Microphone), Output Device (Speakers), modify the boost for both, preview your input audio, connect to a server,  and toggle the radio overlay.

Preview audio will also preview any voice modulation; you hear what the server hears.

#### Controls
Here is where you set up your Key Bindings for SimpleRadio
Each radio slot functions the same; they will be combined below

* Radio # - Set the key/button you want to change the active radio to Radio #
* Radio # Modifier - If this is set, this key must also be pressed in order to change radios
* Common PTT - Button used to enable Push to Talk (PTT)
* Common PTT Modifier - If this is set, this key must also be pressed to enable Push To Talk
* Overlay Toggle - Hide or Display the SimpleRadio overlay. The only Modules that allow the User to interact with the overlay are the FC3 Aircraft, SU-25T and GCI / Spectators Slot
* Overlay Toggle Modifier - If this is set, this key must also be pressed to Hide or Display the SimpleRadio overlay

#### Settings
* Auto Connect Prompt - Enabled Auto-Connect notification, otherwise it will connect silently
* Radio Rx/Tx Effects - Enabled the simulation of squelch & mic click at the start and end of a transmission
* Radio Encryption Effects - Enables simulation of Encryption & Decryption audio tones from a KY-58
* Radio Switch works as PTT - When this button reads 'On', the keybinding used to switch between radios will also act as Push to Talk in addition to switching your active radio. Common PTT will still function
* Radio # Audio Channel - If you have Stereo audio enabled for your Output Device, this determines which channels the relevant radio audio will be sent. Left for the Left Channel, Right for the Right Channel, Both for the Left AND Right channels simultaneously
* Intercom Audio Channel - Same as radio audio channels, but for the intercom on multi-crew ships
* Reset Radio Overlay - Clicking the RESET button will hide the overlay and anchor in the default position on your desktop with full Opacity (fully visible)

## SimpleRadio in DCS
In order to move the SimpleRadio overlay, left-click on a Channel Name or Frequecy and drag.
SimpleRadio WILL NOT FUNCTION if you are not connected to a server; troubleshooting must be done while connected to a SimpleRadio server
#### FC3/Su-25T Modules
In the Lo-Fidelity modules (referred to here as FC3 modules) or spectator / CA / GCI slot there are no In-Game controls for SimpleRadio, everything must be done through the overlay and SimpleRadio keybinds
* Frequency Change - 4 clickable buttons above and below the relevant radio Channel will increase and decrease the frequency respectively
* Volume Control - the slider below a channel controls that channels volume
* Guard - Right click on the frequency of a Channel to enable Guard for the mission specific channel Guard; a G will follow the frequency if this is enabled; the frequency text will turn Red instead of white if receiving on Guard.
* Active Channel - The dot next to the active channel will be Green when no audio is being sent through; it will appear white if audio is being received on that channel; You can make a radio active by clicking on the frequency
* Inactive Channel - The dot next to inactive (but not disabled) channels will be Orange 
* Disabled Channel - The dot next to disabled channels will be Red and either cannot be switched to in any manner while using the select module/aircraft

#### Hi-Fidelity DCS Modules
In Hi-Fidielty DCS modules, DCS-SRS, when properly installed, will link directly to the In-Game Cockpit radio settings. What you see in DCS is what DCS-SRS will follow, this includes Radio Frequencies, Radio Channels, Radio Volume, Channel Guard, etc... 

There are a few exceptions, as not all cockpits have the necessary animations for SRS (namely the Hornet radio switch, M2000C, etc): In order to change the active radio channel, you must use the Keybindings set in the SimpleRadio Client.

The best way to know is to click the `?` in the radio overlay when you are in the airframe of interest.


## Encryption

**NOTICE:** This information is gathered experimentally, so some information
 may be missing, particularly regarding which radios support encryption for the
 in-cockpit aircraft. For aircraft with ODU controlled radios, there may be
 multiple methods to enable encryption; only those tested are described here.

"Radio Encryption Effects" must be enabled in client settings to hear the tones
 described below.

---

In addition to the regular radio communications, SRS supports encrypted radio
 communications (to some degree) in most aircraft. Encrypted radios can still
 understand plain-text voice transmissions, but cannot respond if the recipient
 is not also using encryption (as described below). The "encryption" done by
 SRS is greatly simplified, relying on a single number to determine whether or
 not a radio can "decrypt" an incoming transmission. SRS currently supports the
 values 1-252, although the values usable depends on the aircraft (see below).

There are two main methods to enable encryption, and they vary based on
 aircraft: SRS overlay, or in-cockpit controls. As one may suspect, the only
 aircraft currently with in-cockpit controls are those with full-fidelity
 models.

There is also a relay feature in some aircraft, like in EAM, but these have not
 been tested; the F-14 and F-18 are two such aircraft. Relaying would
 re-transmit across radios, essentially acting as a repeater.

### SRS overlay (method 1)
This first method is the simplest, and is done entirely within the SRS overlay.
 When you load into an aircraft lacking binds/controls for encryption, the SRS
 overlay may add an "Enc" tab adjacent to your radio tab (see image). Aircraft
 without encryption equipment may be able to use encryption, if the "allow
 radio expansion" SRS server option is enabled; a radio with encryption support
 is added to the overlay in this case.

![SRS encryption tab for F-15](https://imgur.com/7kNwB6d.png)

Choose the desired value, ensuring that those who wish to communicate
 securely choose the same value, and press enable. This value can be changed at
 any time, and encryption does not need to be disabled to do so. Verify this
 works by returning to the radio tab, and noting a new `E*` (`*` is value
 chosen in previous step; `E1` for value 1, `E127` for value 127, etc.) in the
 frequency window. Whenever you key an encrypted radio, you will hear a beep,
 indicating the transmission of encrypted voice; do not start talking before
 the beep. Whenever your encrypted radio receives an encrypted communication,
 you will hear a tone, similar to the aforementioned beep; plain-text
 communications will not have this tone.

The radios in External AWACS Mode (EAM) will use this method.

### In-cockpit controls (method 2)

Aircraft in this category allow full (or near full) control of encryption
 through the aircrafts switches or systems. Because of this, the usable key
 values are typically no greater than 1-8 (see aircraft entry for specifics).
 In a future update, there may be an offset value, which can be set in the SRS
 overlay, to supplement these values to allow a larger usable range of values
 for these aircraft. Additionally, encryption may not be available on all
 radios in an aircraft (see aircraft entry for specifics).

The aircraft entries are as follows (excl --> one at a time):

<details>
<summary><h4>A-10C II</h4></summary>


Sourced from: [https://forums.eagle.ru/topic/142491-dcs-simpleradio-standalone/](https://forums.eagle.ru/topic/142491-dcs-simpleradio-standalone/)

Supported radios (all excl): UHF, VHF (FM) <br>
Supported keys (changeable mid-flight): 1-6

1. Power on KY-58 and set mode to OP.
2. Use knob to select desired key.
3. Turn to C/RAD 1 or C/RAD 2, to encrypt desired radio.

Use key knob to change key.

For HAVE QUICK (HQ), turn UHF 100MHz selector to A.
</details>
<details>
<summary><h4>AH-64D</h4></summary>

* Open the COM page.
* At the top, select `UHF`, `FM`, or `HF`. (PB T4, T5, or T6)
* At the bottom, select `SET` (PB B5)
* On the right, R1 to change the `CIPHER`/`PLAIN`, R2 to change the key.

Alternatively you can edit the preset:
* COM page.
* Left column select the preset, then `PRESET EDIT` (B6).
* Then select the radio (UHF, FM, HF) to change, and edit the CIPHER/PLAIN and key to your heart's desires.

</details>

<details>
<summary><h4>AJS-37</h4></summary>

No encryption equipment. Can use SRS overlay (may need expanded radios set on
 SRS server).
</details>
<details>
<summary><h4>AV-8B</h4></summary>

Supported radios (all simul): UHF, VHF/UHF <br>
Supported keys (changeable mid-flight): 1-6

1. Select COMM1 or COMM2 using UFC dials.
2. Press ODU button 4 "PLN" to change to "CIPH".
3. Press ODU button 5 at least once to change the key to a valid one (1-6).

Use ODU button 5 "CD *" (* is key value) to cycle available keys.
</details>

<details>
<summary><h4>Black Shark (2 or 3)</h4></summary>

No encryption.
</details>

<details>
<summary><h4>C-101</h4></summary>

No encryption.
</details>

<details>
<summary><h4>CH-47F</h4></summary>

* Bring up the `CNI` page.
* Select the radio you want to encrypt using the arrow on the left side.
* Change the COMSEC MODE to `CIPHER`

Currently (2.2.0.4) you have to use the overlay to change the key.
</details>

<details>
<summary><h4>F-5E</h4></summary>

Only supports HAVE QUICK (HQ) mode. HQ requires another HQ-enabled aircraft
 to receive communications (EAM cannot use HQ mode). Currently, the F-5E
 and A-10C II are the only HQ-enabled aircraft (that I am aware of).

1. Turn left-most UHF knob to A.
2. Set mode knob to P (preset), to set full frequency.

SRS overlay will show HQ, instead of AM or FM.
</details>
<details>
<summary><h4>F-14 A/B</h4></summary>

**CAUTION:** Requires human in RIO seat, as encryption controls are there.

Supported radios (all excl): UHF, VHF/UHF <br>
Supported keys (changeable mid-flight): 1-8

1. Turn KY-28 knob to C, and select which radio to encrypt (left-side console).
2. Wait for steady tone in selected radio, followed by two-tone beep.

Use ground crew menu (somehow works mid-flight), or kneeboard, to change key.

**NOTICE:** There will now be a 1-2s delay between keying and the beginning of
 transmission (for encrypted radio), so you must hold the PTT button longer
 than w/o encryption.

(F-14 documentation includes steps to enable)
</details>

<details>
<summary><h4>F-16C</h4></summary>

Supported radios (all excl): UHF, VHF/UHF <br>
Supported keys (changeable mid-flight): 1-6

1. Turn on KY-58 power, then switch mode knob to C (back right side panel).
2. Turn key knob clockwise at least once (doesn't start on key 1), to desired key.
3. Move cipher/plain switch to either CRAD1 or CRAD2 (slightly forward of KY-58 panel, right of stick).

Use key knob on KY-58 panel to change key.
</details>
<details>
<summary><h4>F/A-18C</h4></summary>

Supported radios (limited excl): UHF <br>
Supported keys (changeable mid-flight): 1-6

1. Same KY-58 as F-16. See F-16 steps (ignore CRAD1/2 selection).

Use fill/key knob to change keys.
</details>
<details>
<summary><h4>JF-17</h4></summary>

(As of 2.2.0.4):

Using the arrow next to COMM1/2 for the radio you want to change.
Top-left arrow to cycle encryption:
* CMS is encryption only,
* TRS is treated as HQ + encryption.
* PLN is no encryption

Encryption key can be changed by toggling between MAST/SLAV.
</details>
<details>
<summary><h4>L-39</h4></summary>

No encryption equipment. Can use SRS overlay (if expanded radios).
</details>
<details>
<summary><h4>M-2000C</h4></summary>

Supported radios (limited simul): UHF <br>
Supported keys (changeable mid-flight): 1-252

1. Push green UHF CDE switch on UHF panel (toggles encryption on/off).

Use SRS overlay to change key.
</details>
<details>
<summary><h4>Mi-8MTV2</h4></summary>

No encryption.
</details>
<details>
<summary><h4>Mi-24P</h4></summary>

No encryption.
</details>
<details>
<summary><h4>SA342 Gazelle</h4></summary>

**CAUTION:** Easiest with co-pilot, as mode dials are partially obscured
 from pilot view.

Supported radios (all simul): UHF, VHF (FM) <br>
Supported keys (changeable mid-flight): 1-252

For UHF, turn mode selector to "NA" (second mode after 0/off). <br>
For VHF, turn mode selector to "CH.OTAN" (second mode after off)

 Use SRS overlay to change keys.
</details>
<details>
<summary><h4>UH-1H</h4></summary>

No encryption.
</details>

## Roadmap/Wishlist: 
* Server Favourites / History
* Signal / white noise when near the edge of radio range

## Troubleshooting: 
* Backup and then delete your Export.lua from Saved Games\DCS\Scripts, Saved Games\DCS.openalpha\Scripts, Saved Games\DCS.openbeta\Scripts. Then run the installer again.

* Follow the FULL list of instructions here https://forums.eagle.ru/showpost.php?p=2963204&postcount=1122

* Contact me on the forums (http://forums.eagle.ru/showthread.php?t=169387) or (more easily) Discord - https://discord.gg/baw7g3t with a screenshot of you Scripts folder, DCS.log, SimpleRadio logs, your Export.lua, operating system and any antivirus or firewall used and I'll help.

* If you are unable to hook into dcs, delete your DCS-SR Install folder and run the installer again. Your settings WILL be saved.

* If others hear an echo of your voice please read this post: http://forums.eagle.ru/showpost.php?p=2885040&postcount=631 Big thank you to Redacuragsr for solving it

* If your device is not being detected try enabling Allow More Inputs and restarting SRS - this may cause issues with Audio and other inputs. If there are problems - disable "Allow More Inputs" - Open your clientlog.txt and look for the device info like so:

Found Device ID:c537046d-0000-0000-0000-504944564944 USB Receiver Usage: Generic Type: Device 
Found Device ID:c537046d-0000-0000-0000-504944564944 USB Receiver Usage: VendorDefinedBegin Type: Device 
Found Device ID:c33a046d-0000-0000-0000-504944564944 G413 Carbon Mechanical Gaming Keyboard Usage: Consumer Type: Device 
Found Device ID:c33a046d-0000-0000-0000-504944564944 G413 Carbon Mechanical Gaming Keyboard Usage: -189 Type: Device 
Found Device ID:c537046d-0000-0000-0000-504944564944 USB Receiver Usage: VendorDefinedBegin Type: Device 
Found Device ID:c33a046d-0000-0000-0000-504944564944 G413 Carbon Mechanical Gaming Keyboard Usage: -189 Type: Device 
Found Device ID:c537046d-0000-0000-0000-504944564944 USB Receiver Usage: -128 Type: Device 

Find the device you want to use e.g. Found Device ID:c537046d-0000-0000-0000-504944564944 USB Receiver Usage: -128 Type: Device 
Copy the ID c537046d-0000-0000-0000-504944564944
And add into a file called whitelist.txt in your SRS Folder

## Acknowledgements & Thanks: 
* Nossarrian for this :+1: 
* More TODO
* https://github.com/jasl8r/P-Opus
