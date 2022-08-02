# cnc-servcie

This service manages requests sent from the hardware devices such as sign-in events.
It also handles diagnostic tests and adding a hardware device to the account.
It then forwards this sign-in attempts to another service via websockets.

Additionally, it has a web interface that allows for easy administration of devices.


## Backend Methods

* Initialize - Done to add a device to your account (once done, saves to flash)
  When the device first turns on (unconfigured) it can be assiociated to any network.
  It is primed with a set of secret hardware keys that should be released.
  When it encounters these keys it goes into pairing mode.
  * Request sent from device to CNC (via HTTP):
    ```json
    { "kind": "INITIALIZE", "uid": "32 byte string base64", "cardId": "32 byte string" }
    ```
  * Success Response:
    ```json
    { "kind": "INITIALIZE_SUCCESS" }
    ```
  * Failure Response:
    ```json
    { "kind": "INITIALIZE_FAIL" }
    ```
* Startup - Runs whenever an intialized device turns on, or reboots
  * Request (via websocket):
    ```json
    { "kind": "STARTUP", "uid": "32 byte string base64" }
    ```
  * Success response:
    ```json
    { "kind"; "STARTUP_SUCCESS", "apiKey": "some api stuff" }
    ```
  * Failure response:
    ```json
    { "kind": "STARTUP_FAIL" }
    ```
* Command message -  a command sent from the server to the hardware device
  * Request (via websocket):
    ```json
    { "kind": "COMMAND", "commandId": 123, "commandKind: "POWER_CYCLE | FULL_RESET | FLASH | BEEP"}
    ```
  * Response:
    ```json
    { "kind": "COMMAND_ACK", "commandId": 123 }
    ```
* Card Read - happens whenever a card is in close proximity to the sensor
  * Request (via websocket):
    ```json
    { "kind": "CARD_READ", "cardId": 123, "cardPayload": [12, 12, 123] }
    ```
  * Response:
    ```json
    { "kind": "CARD_READ_ACK", "cardId": 123, "sound": "IN | OUT | ACK | ERROR" } 
* ping & pong (included inside websocket protocol)
