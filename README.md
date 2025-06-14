# WebSocket Event Structure

This document specifies the WebSocket event structure for client-server communication within the typing tournament application.

## Core Principles

1.  **Request-Response Model**
    Client-initiated events frequently expect a direct server response, typically `eventName:success` or `eventName:failure`.

2.  **Proactive Server Updates**
    The server may transmit updates to clients without a preceding client request (e.g., `update:me`).

3.  **Tournament Association via Handshake**
    Clients connect to a specific tournament by including the tournament identifier as the `id` query parameter in the WebSocket connection URL. The server utilizes this `id` to associate the client's socket instance with the designated tournament-specific room. Successful association is confirmed by a `join:success` event; failure results in a `join:failure` event.

    _Client-side example snippet for connection:_

    ```javascript
    const tournamentId = "your_tournament_id";
    const socket = io(namespaceUrl, {
      query: { id: tournamentId },
      // ... other options
    });
    ```

4.  **Error Handling**
    All payloads for `:failure` events are of the type `WsFailurePayload` which contains error code and message.

---

## Client-to-Server Events

Events emitted by the client to the server.

| Event   | Description                                                                  | Payload            | Expected Server Response(s)                                |
| ------- | ---------------------------------------------------------------------------- | ------------------ | ---------------------------------------------------------- |
| `me`    | Request the client's own typing session data.                                | `{}`               | `me:success`, `me:failure`                                 |
| `us`    | Request typing session data for all participants in the tournament.          | `{}`               | `us:success`, `us:failure`                                 |
| `leave` | Notify the server of the client's intent to gracefully leave the tournament. | `{}`               | `leave:success`, `leave:failure`                           |
| `type`  | Notify the server that the client has typed a character.                     | `TypeEventPayload` | `type:failure` or eventually triggers a `update:me` event. |
| `data`  | Request comprehensive information about the current tournament.              | `{}`               | `data:success`, `data:failure`                             |
| `check` | Request a status overview of the tournament.                                 | `{}`               | `check:success`, `check:failure`                           |

---

## Server-to-Client Events

Events emitted by the server to client(s).

### 1. Responses to Client Requests

Sent to a specific client in direct response to their requests.

| Event           | Description                                                           | Payload Type          |
| --------------- | --------------------------------------------------------------------- | --------------------- |
| `join:success`  | Confirms successful tournament association and provides initial data. | `JoinSuccessPayload`  |
| `join:failure`  | Indicates failure to associate with the tournament.                   | `WsFailurePayload`    |
| `me:success`    | Returns the client's typing session data.                             | `MeSuccessPayload`    |
| `me:failure`    | Indicates an error fetching client's session data.                    | `WsFailurePayload`    |
| `us:success`    | Returns typing session data for all participants.                     | `UsSuccessPayload`    |
| `us:failure`    | Indicates an error fetching all participants' data.                   | `WsFailurePayload`    |
| `leave:success` | Confirms successful departure from the tournament.                    | `LeaveSuccessPayload` |
| `leave:failure` | Indicates an error during the leave process.                          | `WsFailurePayload`    |
| `type:failure`  | Indicates an error processing the `type` event.                       | `WsFailurePayload`    |
| `data:success`  | Returns comprehensive tournament information.                         | `DataSuccessPayload`  |
| `data:failure`  | Indicates an error fetching tournament information                    | `WsFailurePayload`    |
| `check:success` | Returns the current status of the tournament.                         | `CheckSuccessPayload` |
| `check:failure` | Indicates an error fetching tournament status.                        | `WsFailurePayload`    |

### 2. Proactive Server Updates (Partial Data)

Pushed by the server. Payloads represent partial data from their corresponding `*:success` payloads, containing only changed fields.

#### 2.1. Update to Current Client

Sent to the specific client whose data has changed.

| Event       | Description                                                   | Payload Type      |
| ----------- | ------------------------------------------------------------- | ----------------- |
| `update:me` | Server-initiated partial update of the client's session data. | `UpdateMePayload` |

#### 2.2. Update to All Clients in Room

Sent to all clients within the same tournament room.

| Event         | Description                                                    | Payload Type        |
| ------------- | -------------------------------------------------------------- | ------------------- |
| `update:us`   | Server-initiated partial update of participants' session data. | `UpdateUsPayload`   |
| `update:data` | Server-initiated partial update of overall tournament data.    | `UpdateDataPayload` |

### 3. Broadcast Notifications (Full Data)

Broadcast to all clients in the tournament room. These carry full data payloads.

| Event           | Description                                                | Payload Type          |
| --------------- | ---------------------------------------------------------- | --------------------- |
| `member:joined` | Notifies that a new participant has joined the tournament. | `MemberJoinedPayload` |
| `member:left`   | Notifies that a participant has left the tournament.       | `MemberLeftPayload`   |

---

## Client-Side Event Handling

A recommended pattern for handling client-initiated events expecting a `{eventName}:success` or `{eventName}:failure` response is demonstrated by this `fire` function.

```typescript
async function fire(eventName: PollableEvent, payload: unknown = {}) {
  return new Promise((resolve, reject) => {
    const socket = this.socket;
    const successEvent = `${eventName}:success`;
    const failureEvent = `${eventName}:failure`;
    let timeoutId;

    const cleanup = () => {
      socket.off(successEvent, onSuccess);
      socket.off(failureEvent, onFailure);
      clearTimeout(timeoutId);
    };

    const onSuccess = (data) => {
      cleanup();
      resolve({ success: true, data });
    };

    const onFailure = (error) => {
      cleanup();
      resolve({ success: false, error });
    };

    socket.once(successEvent, onSuccess);
    socket.once(failureEvent, onFailure);

    socket.emit(eventName, payload);

    timeoutId = setTimeout(() => {
      cleanup();
      reject(new Error(`Timeout waiting for response to "${eventName}"`));
    }, 5000);
  });
}
```

### Usage Example

```javascript
try {
  const result = await socketInstance.fire("us", { payload });
  if (result.success) {
    console.log("Received data:", result.data);
  } else {
    console.warn("Error:", result.error);
  }
} catch (error) {
  console.error("Request failed:", error);
}
```

---

## Server-Side Payload Management

The server may use `serde_json::json!` for dynamic construction of JSON response payloads, particularly for structures not requiring predefined Rust structs and for partial updates.

```rust
use serde_json::json;

let payload = json!({
    "userId": user_id,
    "tournamentId": room_id,
    "message": "Successfully joined the tournament."
});
socket.emit("join:success", payload).ok();
```

### Payload types

```rust
pub struct TypeArgs {
    character: char,
}
```

---

## Payload Type Definitions (TypeScript)

These define the structure of data exchanged for various events.

Http endpoint `/auth/me` returns `ClientSchema` whether user is authenticated or not. This helps to track each client in tournaments.

### Common Types

```typescript
export type UserSchema = {
  id: string;
  username: string;
  email: string;
  createdAt: string;
  updatedAt: string;
};

export type ClientSchema = {
  id: string;
  user: UserSchema | null; // some users are anonymous (playing without loggin in)
  updated: string;
};

export type TextOptions = {
  uppercase: boolean; // includes uppecase letters
  lowercase: boolean; // includes lowercase letters
  numbers: boolean; // includes numbers
  symbols: boolean; // includes special characters
  meaningful: boolean; // if words or constructions are meaningful
};

export type TournamentData = {
  id: string;
  title: string;
  createdAt: string;
  createdBy: string;
  scheduledFor: string;
  description: string;
  textOptions: TextOptions;
  privacy: string;
  startedAt: string | null;
  endedAt: string | null;
  text: string | null;
};

export type ParticipantData = {
  client: ClientSchema;
  currentPosition: number;
  correctPosition: number;
  totalKeystrokes: number;
  currentSpeed: number;
  currentAccuracy: number;
  startedAt: string | null; // specific to the participant
  endedAt: string | null; // specific to the participant
};

export type ParticipantUpdate = {
  updates: Partial<Omit<ParticipantData, "client">>;
};

export type WsFailurePayload = {
  code: number;
  message: string;
};

export type PollableEvent = "me" | "us" | "data" | "check" | "leave";

export type TypeEventPayload = {
  character: string; // single character typed by the user
};
```

### `JoinSuccessPayload`

```typescript
export type JoinSuccessPayload = {
  data: TournamentData;
  clientId: string; // id for the participant. equivalent to `client.id`
  participants: ParticipantData[];
};
```

### `MeSuccessPayload`

Payload for `me:success`.

```typescript
export type MeSuccessPayload = ParticipantData;
```

### `UpdateMePayload`

Payload for `update:me` (partial `MeSuccessPayload`). `userId` is implicit.

```typescript
export type UpdateMePayload = ParticipantUpdate;
```

### `UsSuccessPayload`

Payload for `us:success`.

```typescript
export type UsSuccessPayload = ParticipantData[];
```

### `UpdateUsPayload`

Payload for `update:us`.

```typescript
type PartialParticipantDataForUpdate = {
  clientId: string;
} & ParticipantUpdate;

export type UpdateUsPayload = {
  updates: PartialParticipantDataForUpdate[];
};
```

### `DataSuccessPayload`

Payload for `data:success`.

```typescript
export type DataSuccessPayload = TournamentData;
```

### `UpdateDataPayload`

Payload for `update:data` (partial `DataSuccessPayload`).

```typescript
export type UpdateDataPayload = Partial<
  Omit<TournamentData, "id" | "createdAt" | "createdBy">
>;
```

### `CheckSuccessPayload`

Payload for `check:success`.

```typescript
export type CheckSuccessPayload = {
  status: "upcoming" | "started" | "ended";
};
```

### `MemberJoinedPayload`

Payload for `member:joined`.

```typescript
export type MemberJoinedPayload = {
  participant: ParticipantData;
};
```

### `MemberLeftPayload`

Payload for `member:left`.

```typescript
export type MemberLeftPayload = {
  clientId: string;
};
```

### `LeaveSuccessPayload`

Payload for `leave:success`.

```typescript
export type LeaveSuccessPayload = {
  message: string;
};
```

## Client-side State Management

### State management types

1.  `participants`: `Record<string, ParticipantData>`

- Initially got from `JoinSuccessPayload.participants` (recommended) or `UsSuccessPayload`
- Key for the record will be client id accessed from `client.id` or `JoinSuccessPayload.clientId`.
- Must be updated after each `update:us` event.
- Must be manually updated after `member:left` and `member:joined` as there will not be a `update:us` for these cases.
- Must be updated after every `update:me` event unless implementation separates `me` from `participants`,
  in which case `me` will be updated separately and `participants` will catch up with the next `update:us` event.

2.  `data`: `TournamentData`

- Initially got from `JoinSuccessPayload.data` (recommended) or `DataSuccessPayload`
- Must be updated after every `update:data` event.

3.  `me`: `ParticipantData` (If stored as a separe object).

- Initially got from `participants[JoinSuccessPayload.clientId]` (recommended) or `MeSuccessPayload`
- Must be updated after every `update:me` event.

4.  `TournamentData.text` and `TournamentData.startedAt` will simultaneously be set when tournament starts via a `update:data` event.
5.  Server may process multiple successive `type` events together and return the updates in a single `update:me` event.

### Web socket implementation notes

1. Abstract web socket logic into a class(es) from which an instance can be created once and passed around
2. Use a single socket connection per client, and reuse it for all events.
3. Reconnect logic should lead to state updates (via the `join:success` event callback)

### Retries

- Reconnecting with socket after disconnect will recover previous data
- Reconnecting logic should always wait for `join:success` and update state variables
- All **pollable events** can be retried any number of times.

\_(**pollable events** follow the pattern: client fires `{eventname}` then responds with `{eventname}:success` or `{eventname}:failure`. They can be implemented as promises that resolve after server responds)

## Error Codes

All `:failure` events return a `WsFailurePayload` object:
`{ code: number; message: string; }`

### 1xxx: Connection & Handshake Errors
| Code | Event(s)       | Default Message Suggestion                      | Notes                                     |
|------|----------------|-------------------------------------------------|-------------------------------------------|
| 1001 | `join:failure` | "Tournament ID missing in handshake query."     | `id` query param not provided.            |
| 1002 | `join:failure` | "Invalid Tournament ID format."                 | `id` query param has incorrect format.    |
| 1003 | `join:failure` | "Tournament not found."                         | The specified tournament does not exist.  |
| 1004 | `join:failure` | "Maximum participants reached."                 | Tournament is full.                       |
| 1005 | `join:failure` | "Already connected to this tournament."         | If single session per user is enforced.   |
| 1006 | `join:failure` | "Access denied to private tournament."          |                                           |

### 2xxx: Client Request & Validation Errors
| Code | Event(s)       | Default Message Suggestion                      | Notes                                     |
|------|----------------|-------------------------------------------------|-------------------------------------------|
| 2001 | *Any*          | "Invalid event name."                           | Server received an unrecognized event.    |
| 2002 | *Any*          | "Malformed request payload."                    | e.g., Invalid JSON.                       |
| 2101 | `type:failure` | "Invalid payload: 'character' missing/invalid." | `TypeEventPayload` validation failed.     |
| 2201 | `type:failure` | "Typing not allowed: Tournament not started."   |                                           |
| 2202 | `type:failure` | "Typing not allowed: Tournament ended."         |                                           |
| 2203 | `type:failure` | "Typing not allowed: You have finished."        |                                           |
| 2210 | *Most C2S*     | "Not joined to a tournament."                   | Action requires being in a tournament.    |

### 3xxx: Resource & State Errors
| Code | Event(s)       | Default Message Suggestion                      | Notes                                     |
|------|----------------|-------------------------------------------------|-------------------------------------------|
| 3001 | `data:failure` | "Tournament data unavailable."                  | Error fetching overall tournament details.|
| 3101 | `me:failure`, `us:failure` | "Participant data unavailable."     | Error fetching participant details.       |
| 3102 | `leave:failure`| "Failed to process leave request."              |                                           |


### 4xxx: Server-Side Operational Errors
| Code | Event(s)       | Default Message Suggestion                      | Notes                                     |
|------|----------------|-------------------------------------------------|-------------------------------------------|
| 4000 | *Any*          | "Internal server error."                        | Generic server-side issue.                |
| 4001 | *Any*          | "Operation timed out on server."                |                                           |
| 4101 | *Any*          | "Database error."                               | Problem with data persistence.            |

_(This list is not exhaustive and should be expanded as specific error cases are identified during development.)_

## Future Improvements

- Add a `cursor` field to `type` event payloads, and resolution logic for common inconsistencies