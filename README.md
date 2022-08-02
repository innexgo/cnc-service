# cnc-servcie

This service manages requests sent from the hardware devices such as sign-in events.
It also handles diagnostic tests and adding a hardware device to the account.
It then forwards this sign-in attempts to another service via websockets.

Additionally, it has a web interface that allows for easy administration of devices.


## Methods for communicating with the hardware device

* Register - Done to add a device to your account (once done, saves to flash)
  When the device first turns on (unconfigured) it can be registered to any network.
  It is primed with a set of secret hardware keys that should not be released.
  When it encounters these keys it goes into pairing mode.
  * Request sent from device to CNC (via HTTP):
    * `https://<host>/public/register`
    ```json
    { "kind": "REGISTER", "uid": "32 byte string base64", "supervisorCardId": "32 byte string" }
    ```
  * Success Response:
    ```json
    { "kind": "REGISTER_SUCCESS" }
    ```
  * Failure Response:
    ```json
    { "kind": "REGISTER_FAIL" }
    ```
* Startup - Runs whenever an register device turns on, or reboots, or right after an hardware registers
  * Request (via websocket):
    * `wss://<host>/public/websocket`
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
* Command message -  a command sent from the CNC server to the hardware device
  * Request (via websocket):
    * `wss://<host>/websocket`
    ```json
    { "kind": "COMMAND", "commandId": 123, "commandKind": "POWER_CYCLE | FULL_RESET | FLASH | BEEP"}
    ```
  * Success Response:
    ```json
    { "kind": "COMMAND_ACK", "commandId": 123 }
    ```
* Card Read - sent from device to CNC whenever a card is in close proximity to the sensor
  * Request (via websocket):
    * `wss://<host>/public/websocket`
    ```json
    { "kind": "CARD_READ", "cardReadId": 123, "cardPayload": [12, 12, 123] }
    ```
  * Success Response:
    ```json
    { "kind": "CARD_READ_ACK", "cardReadId": 123, "sound": "IN | OUT | ACK | ERROR | TIMED_OUT" }
    ```
  * Failure Response:
    ```json
    { "kind": "NO_STARTUP" }
    ```
* ping & pong (included inside websocket protocol)


## Methods for Communicating with Another Microservice
This allows you to watch for card reads, and forward commands from another microservice.

* Forward Card Read - sent from CNC to a listening microservice when a card read is recieved
  * Request sent from CNC (via websocket):
    * `wss://<host>/feed`
    ```json
    { "kind": "CARD_READ", "deviceId": 123, "cardReadId": 123, "cardPayload": [12, 12, 123] }
    ```
    ```json
    { "kind": "INITIALIZE", "deviceId": 123, "uid": "32 byte string base64", "supervisorCardId": "32 byte string" }
    ```
  * Response sent from microservice:
    ```json
    { "kind": "CARD_READ_ACK", "deviceId": 123, "cardReadId": 123, "sound": "IN | OUT | ACK | ERROR" }
    ```

* Forward Command - sent from a microservice to CNC
  * Request from a microservice to the CNC (via http):
    * `https://<host>/command`
    ```json
    { "deviceId": 123, "commandKind": "POWER_CYCLE | FULL_RESET | FLASH | BEEP"}
    ```
  * Success Response:
    ```json
    {}
    ```
  * Failure Reponse:
    ```json
    "{ "kind": "DEVICE_NONEXISTENT" }"
    ```
  * Failure Reponse:
    ```json
    "{ "kind": "DEVICE_TIMED_OUT" }"
    ```

* Device Query - allows the microservice to see which devices are registered
  * Request sent from a microservice to CNC
