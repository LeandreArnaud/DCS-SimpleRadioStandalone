![Main screen on first startup](https://github.com/user-attachments/assets/32e71c40-a09a-4bad-a1e8-96fda27cf8c8)

# Elements of main screen

## Microphone

Microphone selection box - used to select which input device to use with SRS  
Grey audio preview box - visual representation of audio volume in blue when audio preview is enabled  
**audio preview** click button to toggle on/off audio preview - when on audio picked up by the mic wil be played back through the output device (for testing etc..)  

## Speakers & Optional Mic Output

The first selection box is to select which output device you'd like SRS to use to output comms audio.  
The second selection box is to select an option output device for passing through audio captured by the selected mic (possibly for recording etc??).  
The slider can be used to add a positive or negative adjustment to audio.  
The bar provides a visual representation of audio volume

## Server
The left entry box will allow entering of server IP manually, syntax is IP:PORT, if port is not specified the defailt port is used eg; 123.456.12.34:1234  
This box will be automatically populated if auto connect is enabled.  
The right slection box allows selecting from servers saved in the 'Favorites' screen or previously connected servers(correct???)  
**connect** button triggers an attempt to connect to a server address selected or entered above.  
**show server settings** button displays the settings in effect on the connected server.

## Additional buttons
**toggle radio overlay** toggles on/off the SRS radio overlay  
**toggle AWACS overlay** toggles on/off the SRS AWACS overlay  
**toggle client list** toggles on/off a list of users connected the the current SRS server

## Presets

By default, all presets are saved next to the client executable.
This is the `awacs.json`, `awacs-custom.json` as well as channelization text files.

See the [Client Presets page](../Presets) for more information.

The `Browse Presets folder` lets you pick a folder containing all the files, anywhere on your machine.
This is an organization tool to be able to maintain different presets easily and not lose them on updates.

The `Reset` button will... reset (surprise!) the field to load from the SRS directory.

It is especially useful if you're often controlling on different servers, giving you the ability to quickly switch
radio configurations and channelization for your EAM AWACS radio panel.

## External AWACS Mode (EAM)

![EAM expanded](https://github.com/user-attachments/assets/eca4db4f-29f9-4046-b6c4-0c40af18c132)

Allows to be connected without being in an aircraft in DCS.
For instance you can connect via [LotAtc](https://www.lotatc.com/) to provide ATC/GCI services,
or even use it with a completely different game.

**EAM coalition password** EAM password set by the server

**EAM coalition name** Your display name as EAM

**connect external AWACS mode (eam)** connects to SRS server as EAM.

## Footer section

**Current Profile:** shows name of the current SRS profile in use

**Unit** Shows the unit name you're in as seen by SRS. Useful if you're looking to make per aircraft profiles and don't know the unit's name.

**Connected clients:  &  Current Position:** shows number of clients connected to SRS server  &  shows users position in game

## **Connection status** Shows connection health status to relative components, Server, VOIP , Game and [LotAtc](https://www.lotatc.com)  