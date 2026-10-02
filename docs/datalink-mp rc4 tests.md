Linux-Linux: Mint on wifi, Rocky on mobile data hotspot

1. Host=Mint smacl5b7qg2r432ubaoyl523asszofp4ht3wnfdts2zw55rockwjiboakaiavqiqaanx4abacafmciaadn7aaiaqbqfiwim3pyacaeasaayaz27qeryaf7j2xtbif2wlhzpoaiaqciadadhl6ashadzu7is6qeic4pxf5yba  
     
* Iroh connected: in joined Helper, it checks and says “Connected to your friend”, this is good; in host Helper, we should consider moving the “\# friends connected” also to the 2nd stage  
* If possible, can our Helper recognize for the Joiners when Host has MP ready? If Joiners try to join before that time, it fails with “game not found”. Can Host Helper somehow automatically signal, or could Host player manually signal? If not, they will somehow need to communicate through other means (not a problem, they are already somehow passing Tickets to each other). In all cases, the Joiners Helper should not simply say at Stage 3 “Start the game”, but rather something like “Wait for host to tell you game is ready, then start game”

* Game found; both Host and Joiner could adjust ui elements with no issues;Host name was visible in UI, Joiner name was blank despite name being set; Joiner pressed cancel, then pressed multiplier \-\> iroh \-\> join, but this time “no game found”.  
* Joiner closed game, stopped and quit Helper, reopened Helper, re-pasted Ticket from still-running Host (never interrupted, still in-game sitting in MP setup), but again “no game found”  
* Both Helpers stopped and Quit, end of Test 1

2. Host=Mint  
   Smachnindyc2ajyvm4xx6imfmtojmzglsiadinkrszrfrntlsjuwactqkaiavqiqaan6v4bqcafmciaadpvpamaqbqfiwim35lydaeasaayaz27qeryaf7j2xtbif2wlhrv4amaqciadadhl6ashadzu7is6qeic4pwgxqbq  
     
* Iroh connected, Joiner found game, Joiner name still not visible in ui (see image)  
* Trying “Simultaneous moves”, also Joiners must click “Make Ready” before Host can start game (we should consider noting this in our Helper)  
* Host froze, game dropped (unreleated to our app, happens in single player on this box also, troubleshooting before next test)  
* Both Helpers stopped and Quit, end of Test 2  
3. Host=Mint (now working when “smooth\_scrolling=0” in thinker.ini  
   Smacyfzhktx7dovimzp7ctpi46uk5fzjqxqafeebd5wcxv2tyhufyp7qkaiavqiqaaoywibqcafmciaadwfsamaqbqfiwim5rmqdaeasaayaz27qeryaf7j2xtbif2wlhbvqaiaqciadadhl6ashadzu7is6qeic4pugwaba  
     
* Iroh connected, Joiner found game, Joiner name not visible  
* Simultaneous moves game launched, all works well. Seems much faster than internet game with friend (from Eastern Europe \- Germany, versus German Telekom DSL \- Mobile Data)

Windows-Linux
Test 4: Host=Windows11 (Wifi) Joiner=Rocky (Mobile Data Hotspot)
smacqgcoiuvrovkp7c7sjv5th7fsgvvmudo6qakiu5b5j3pnbp3hh2pagaiayculegm2yybqcajaamam5pyci4aa4ozkmocjvijhttdagaibeabqbtv7ajdqbkno5wkbf6fk4gommay

Iroh connected, game found (no Joiner name) and launched
Played a few turns, connection speed comparable to Test 4
Ending Test 5, Joiner quit game (possibly no notification on Host), then when Host clicked “End Game” white Net::send Null Pointer pop-up (my guess is Host had not yet registered Joiner disconnect, attempted to disconnect Joiner and they were already gone)

