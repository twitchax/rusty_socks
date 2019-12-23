* Don't hardcode the read timeout.
* Transport endpoint is not connected (os error 107): probably caused by the write timing out...we should fail gracefully there?
* Convert errors and message types to strings...enum?