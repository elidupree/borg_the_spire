# Borg the Spire (under construction)

An AI/cyborg gameplay helper for Slay the Spire. Runs alongside Slay the Spire and shows an external window with AI suggestions.

Fair warning: This is a toy project for me, and I don't particularly expect to complete it or make it convenient for anyone but me to use.

This crate compiles to an executable usable with [CommunicationMod](https://github.com/ForgottenArbiter/CommunicationMod).

To use:
* `cargo build`
* Set the CommunicationMod command to point to the compiled executable, with one command line argument that is a path to the Borg the Spire directory (e.g. `command=C\:\\Users\\Eli\\Documents\\borg_the_spire\\target\\debug\\borg_the_spire.exe C\:\\Users\\Eli\\Documents\\borg_the_spire\\`)
* Run Slay the Spire with mods, enabling CommunicationMod
* While Slay the Spire is running, go to `http://localhost:3508/` in a browser for the interface