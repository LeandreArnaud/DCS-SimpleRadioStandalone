This page is aimed at Dedicated Server setups. If you find any information in this wiki/documentation which requires correction or updating, please leave a comment, contact one of the contributors, or leave a message on the [SRS discord](https://discord.com/channels/298054423656005632/473947464349646848).

# Contents
* [Prerequisites](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki/SRS-Server-(WIP)#srs-server-prerequisites)
* [Installation](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki/SRS-Server-(WIP)#srs-server-installation)
* [Configuration](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki/SRS-Server-(WIP)#srs-server-configuration)
* [GUI Walkthrough](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki/SRS-Server-(WIP)#srs-server-gui-walkthrough)
* [Methods of Starting SR-Server](https://github.com/ciribob/DCS-SimpleRadioStandalone/wiki/SRS-Server-(WIP)#methods-of-starting-sr-server)
* [Automatic Updating]()

## SRS Server Prerequisites
_**W.I.P.**_
In order to host a DCS-SimpleRadio server, you must forward the following TCP and UDP port 5002. Once the necessary ports are open, simply run the 'SR-Server.exe'. It does not have to be run on a PC running DCS! You can edit the default server port by editing the server.cfg.

If you edit - make sure to update your port forwarding and always forward both protocols TCP and UDP.

## SRS Server Installation

The simplest way to install is to run the SRS Auto Updater - and when prompted if you want to change settings - say Yes.

On the installation window - untick install scripts as these are only required for clients.

Optionally, run with `-server` argument, which will skip the client install.

## SRS Server Configuration
_**W.I.P.**_
#### Auto Connect Feature - Server Owners ONLY - Clients don't need this file!

To enable SRS clients to be prompted automatically to connect just add the DCS-SRS-AutoConnectGameGUI.lua to the appropriate DCS Saved Games folder e.g. DCS.openbeta/Scripts/**Hooks**, DCS.openalpha/Scripts/**Hooks** or jut DCS/Scripts/**Hooks**

Edit the line shown below in your DCS-SRS-AutoConnectGameGUI.lua to your server address where SRS server is running. Port is optional. DCS must be restarted on the server for this file and any changes to take effect.

```lua
-- CHANGE FROM
SRSAuto.SERVER_SRS_HOST = "127.0.0.1" -- Port optional e.g. "127.0.0.1:5002"

--TO
SRSAuto.SERVER_SRS_HOST = "5.189.162417:5014" -- BuddySpike One
-- OR
SRSAuto.SERVER_SRS_HOST = " 37.59.10.136" -- TAW One (port optional)
```
#### Auto Export List: 
Outputs JSON every 30 seconds to clients-list.json. This can be used to show what users are connected to the server on a separate webpage or app.

If this is your first time hosting a server, there are plenty of guides on the internet already dedicated to port forwarding! If you believe you have your ports forwarded and are still unable to get users connected to your server, make sure you have forwarded the ports on EVERY ROUTER between you and your ISP. Many times, your ISP will have a sort of virtual router installed or a physical router specific to your connection, somewhere in the physical node of your area. You are typically free to edit these, though the IP address to connect to them can vary. Call your ISP if you believe this is the case and cannot find the relevant IP to connect through.
## SRS Server GUI Walkthrough
_**W.I.P.**_
![](https://github.com/ciribob/DCS-SimpleRadioStandalone/blob/master/docs/SRS_ServerGUI.png)

#### 1. Start/Stop Server
Starts and stops the SRS server.
#### 2. Clients
The number of SR-Clients connected to the server.
#### 3. Port
The Port the server is listening on.
#### 4. Client Admin
View names of connected clients; The names of the clients will be displayed as follows:
* <whitespace> if the Client is not in a DCS mission
* 'player' if the Client is in a Single Player DCS mission
* <DCS Multiplayer name> if the Client is in a Multi Player DCS mission
Kick a client to kick that client
Ban a client to ban that client
#### 5. Secure Coalition Radios On/Off
When turned ON - Each coalition can only hear other radios of the same Coalition. I.e. Red to Red, Blue to Blue, Spectator to Spectator

When turned OFF - Any coalition can hear any other coalitions radio. To keep your radio secure from eavesdropping, don't publish your frequencies to the enemy or if available, use encryption :+1: 
#### 6. Spectator Audio
When the button shows 'Enabled' spectators are given the ability to use SimpleRadio as if they were in a FC3 ship and may send/receive on any channel they select. 
#### 7. Auto Export List
Exports the player list to clients-list.json file in the SRS folder.
#### 8. Line of Sight
Radios will only work if they have line of sight of one another.
#### 9. Distance Limit
Depending on the strength of the signal the radios might not have enough range to recieve signals.
#### 10. IRL Radio Tx Behaviour
Radio will simulate real life behaviour of that specific radio when transmitting.
#### 11. IRL Radio Rx Behaviour
Radio will simulate real life behaviour of that specific radio when recieving.
#### 12. Radio Expansion
This will add additional radios for aircraft that either did not have them or only had a single radio.
#### 13. Allow Radio Encryption
This enables encryption for blue and red teams to stop them from being able to hear each other if tuned to the same frequency.
#### 14. Strict Radio Encryption
When set, radios with encryption enabled will be unable to process unencrypted transmissions and unecrypted transmissions received will be output as noise.
#### 15. Check for beta updates
When updating the server will include the beta branches for updates.
#### 16. External AWACS Mode (EAM)
This enables external DCS players to communicate as an AWACS
#### 17. EAM blue coal. password
Blue Force password for EAM connections
#### 18. EAM red coal. password
Red Force password for EAM connections
#### 19. Test Frequencies AM (Mhz)
SRS test frequencies
#### 20. Show Tuned/Client Count
When in the sim, it will show the number of clients tuned into that specific frequency. Disable this feature if you do not wish for players to get this information through SRS.
#### 21. Global Lobby Freq. AM (Mhz)
This frequency will be audible by both teams simultaniously.
#### 22. Ignore Radio Effect on Global Freq
Transmissions made on global lobby frequencies will not have radio effects (high/low-pass filters, distance interference) applied.
#### 23. LotATC Transponder Export
This will allow for transponder beeps to be broadcast through SRS from LotATC.
#### 24. Show Transmitter Name
This will show the pilot player or AWACS player that is speaking on the radio.
#### 25. Log Received Transmissions
All transmissions made by connect clients will be logged to ```/SRS-Server Location/YYYY-MM-DD-transmissionlog.CSV```, with a new log created daily.
#### 26. Transmission Archive Time 
The amount of days logs will be retained for before being automatically deleted.
#### 27. Retransmit Hop/Node Count
The amount of retrans hops a broadcast can travel through.

## Methods of Starting SR-Server
_**W.I.P.**_
In this section we will cover three methods of initialization for SR-Server.

### Manual Server start
The SRS Server can be started by simply double clicking on the SR-Server.exe to start.

### PowerShell Initialization Methods (Single Instance)
PowerShell stores data in variables, variables are identifiable by the preceeding '$'. 

**$srsDIR** variable is the location of your SRS installation.

**$srsEXE** variable uses the $srsDIR variable and adds \SR-Server.exe to signify the executable for the SRS Server.

_**start-process**_ is the PowerShell command that will execute SR-Server.exe and start your SRS Server. This command needs to know where the executable is ($srsEXE) and where the program will be working ($srsDIR)

Create a File called Start_SRS.ps1, open the file in your code / text file editor of choice. Copy the Code snippet below into the file. 
```powershell
$srsDIR = "<INSERT YOUR SRS INSTALL FOLDER HERE>"
$srsEXE = "$srsdir\SR-Server.exe"
start-process $srsexe -WorkingDirectory $srsDIR
```
***
### PowerShell Initialization Methods (Mutli-Instance)
In addition to the Single Instance Variables, Multi-Instance deployments need some additional variables. 

**$srsCONFIGFile** variable is the location of configuration file for this instance of SR-Server you are trying to run. e.g. **$srsCONFIGFile= "server02.cfg"**

**$srsARGS** variable is a set of arguments thrown to the start-process command, this is to call out the '-cfg=<path to config file>' argument.

_**start-process**_ is the PowerShell command that will execute SR-Server.exe and start your SRS Server. This command needs to know where the executable is ($srsEXE) and where the program will be working ($srsDIR)

Create a File called Start_SRS.ps1, open the file in your code / text file editor of choice. Copy the Code snippet below into the file. 
```powershell
$srsDIR = "<INSERT YOUR SRS INSTALL FOLDER HERE>"
$srsCONFIGFile= "<insert your config file name here>"
$srsARGS= "-cfg=`"$SRS_Config`""
$srsEXE = "$srsdir\SR-Server.exe"
start-process $srsexe -ArgumentList $srsARGS -WorkingDirectory $srsdir
```
***