# WebSocket Event Structure

This document specifies the WebSocket event structure for client-server communication within the typing tournament application.

## Core Principles

1.  **Request-Response Model**
    Client-initiated events frequently expect a direct server response, typically `eventName:success` or `eventName:failure`.

2.  **Proactive Server Updates**
    The server may transmit updates to clients without a preceding client request (e.g., `update:me`).

3.  **Tournament Association via Handshake**
    Clients connect to a specific tournament by including the tournament identifier (`id`) query parameter in the WebSocket connection URL. To join as a **spectator**, an additional `spectator=true` query parameter should be included. The server uses the `id` to associate the client's socket instance with the tournament-specific room.

    - Spectator sockets, while in the same room for broadcasts, will not have server-side listeners registered for participant-specific events (e.g., `type`, `me`).
    - Successful association is confirmed by a `join:success` event. For spectators, their `ParticipantData` will be absent from the `JoinSuccessPayload.participants` list.
    - Failure results in a `join:failure` event.

    _Client-side example (participant):_

    ```javascript
    const tournamentId = "your_tournament_id";
    const socket = io(namespaceUrl, {
      query: { id: tournamentId },
      // ... other options
    });
    ```

    _Client-side example (spectator):_

    ```javascript
    const tournamentId = "your_tournament_id";
    const socket = io(namespaceUrl, {
      query: { id: tournamentId, spectator: "true" },
      // ... other options
    });
    ```

4.  **Error Handling**
    All payloads for `:failure` events are of the type `WsFailurePayload`, containing an error code and message. For events where a spectator's socket has no server-side listener, client-side requests for such events will not receive an application-level `eventName:success` or `eventName:failure` response, potentially resulting in a client-side timeout if a response is awaited.

---

## Client-to-Server Events

Events emitted by the client to the server.

| Event   | Description                                                                  | Payload            | Expected Server Response(s) for Participants                | Notes for Spectators                                                            |
| ------- | ---------------------------------------------------------------------------- | ------------------ | ----------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `me`    | Request the client's own typing session data.                                | `{}`               | `me:success`, `me:failure`                                  | No server-side listener. Client will not receive an application-level response. |
| `all`   | Request typing session data for all participants in the tournament.          | `{}`               | `all:success`, `all:failure`                                | Permitted. Server listens and responds.                                         |
| `leave` | Notify the server of the client's intent to gracefully leave the tournament. | `{}`               | `leave:success`, `leave:failure`                            | Permitted. Server listens and responds.                                         |
| `type`  | Notify the server that the client has typed a character.                     | `TypeEventPayload` | `type:failure` or eventually triggers an `update:me` event. | No server-side listener. Client will not receive an application-level response. |
| `data`  | Request comprehensive information about the current tournament.              | `{}`               | `data:success`, `data:failure`                              | Permitted. Server listens and responds.                                         |
| `check` | Request a status overview of the tournament.                                 | `{}`               | `check:success`, `check:failure`                            | Permitted. Server listens and responds.                                         |

---

## Server-to-Client Events

Events emitted by the server to client(s).

### 1. Responses to Client Requests

Sent to a specific client in direct response to their requests (if a server-side listener for the event and client type exists).

| Event           | Description                                                            | Payload Type          |
| --------------- | ---------------------------------------------------------------------- | --------------------- |
| `join:success`  | Confirms successful tournament association and provides initial data.  | `JoinSuccessPayload`  |
| `join:failure`  | Indicates failure to associate with the tournament.                    | `WsFailurePayload`    |
| `me:success`    | Returns the client's typing session data (Participants only).          | `MeSuccessPayload`    |
| `me:failure`    | Indicates an error fetching client's session data (Participants only). | `WsFailurePayload`    |
| `all:success`   | Returns typing session data for all participants.                      | `AllSuccessPayload`   |
| `all:failure`   | Indicates an error fetching all participants' data.                    | `WsFailurePayload`    |
| `leave:success` | Confirms successful departure from the tournament.                     | `LeaveSuccessPayload` |
| `leave:failure` | Indicates an error during the leave process.                           | `WsFailurePayload`    |
| `type:failure`  | Indicates an error processing the `type` event (Participants only).    | `WsFailurePayload`    |
| `data:success`  | Returns comprehensive tournament information.                          | `DataSuccessPayload`  |
| `data:failure`  | Indicates an error fetching tournament information                     | `WsFailurePayload`    |
| `check:success` | Returns the current status of the tournament.                          | `CheckSuccessPayload` |
| `check:failure` | Indicates an error fetching tournament status.                         | `WsFailurePayload`    |

### 2. Proactive Server Updates (Partial Data)

Pushed by the server. Payloads represent partial data from their corresponding `*:success` payloads, containing only changed fields.

#### 2.1. Update to Current Client (Participants Only)

These events exist for the sake of a pleasant typing experience.
They
are sent to the **participant** client whose data has changed. Spectators do not receive this event.

| Event       | Description                                                        | Payload Type      |
| ----------- | ------------------------------------------------------------------ | ----------------- |
| `update:me` | Server-initiated partial update of the participant's session data. | `UpdateMePayload` |

#### 2.2. Update to All Clients in Room (Including Spectators)

Sent to all clients (participants and spectators) within the same tournament room.

| Event         | Description                                                    | Payload Type        |
| ------------- | -------------------------------------------------------------- | ------------------- |
| `update:all`  | Server-initiated partial update of participants' session data. | `UpdateAllPayload`  |
| `update:data` | Server-initiated partial update of overall tournament data.    | `UpdateDataPayload` |

### 3. Broadcast Notifications (Full Data) (Including Spectators)

Broadcast to all clients (participants and spectators) in the tournament room. These carry full data payloads.

| Event           | Description                                                    | Payload Type          |
| --------------- | -------------------------------------------------------------- | --------------------- |
| `member:joined` | Notifies that a new **participant** has joined the tournament. | `MemberJoinedPayload` |
| `member:left`   | Notifies that a **participant** has left the tournament.       | `MemberLeftPayload`   |

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
  const result = await socketInstance.fire("eventName", payload);
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
    "userId": "user_id_example",
    "tournamentId": "room_id_example",
    "message": "Successfully joined the tournament."
});

socket.emit("join:success", payload).ok();
```

### Payload types (Rust example)

```rust
struct TypeEventPayload {
    character: char,
}
```

---

## Payload Type Definitions (TypeScript)

These define the structure of data exchanged for various events.

The HTTP endpoint `/auth/me` returns `ClientSchema` whether the user is authenticated or not. This helps track each client in tournaments.

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
  user: UserSchema | null;
  updated: string;
};

export type TextOptions = {
  uppercase: boolean;
  lowercase: boolean;
  numbers: boolean;
  symbols: boolean;
  meaningful: boolean;
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
  startedAt: string | null;
  endedAt: string | null;
};

export type ParticipantUpdate = {
  updates: Partial<Omit<ParticipantData, "client">>;
};

export type WsFailurePayload = {
  code: number;
  message: string;
};

export type PollableEvent = "me" | "all" | "data" | "check" | "leave";

export type TypeEventPayload = {
  character: string;
};
```

### `JoinSuccessPayload`

```typescript
export type JoinSuccessPayload = {
  data: TournamentData;
  clientId: string;
  participants: ParticipantData[];
};
```

### `MeSuccessPayload`

Payload for `me:success`.

```typescript
export type MeSuccessPayload = ParticipantData;
```

### `UpdateMePayload`

Payload for `update:me` (partial `MeSuccessPayload`). `clientId` is implicit.

```typescript
export type UpdateMePayload = ParticipantUpdate;
```

### `AllSuccessPayload`

Payload for `all:success`.

```typescript
export type AllSuccessPayload = ParticipantData[];
```

### `UpdateAllPayload`

Payload for `update:all`.

```typescript
type PartialParticipantDataForUpdate = {
  clientId: string;
} & ParticipantUpdate;

export type UpdateAllPayload = {
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

---

## Client-Side State Management

### State Management Types

1.  **`participants`: `Record<string, ParticipantData>`**

    - Initially populated from `JoinSuccessPayload.participants` or `AllSuccessPayload`.
    - The key for the record will be the `client.id` from `ParticipantData`.
    - Must be updated after each `update:all` event.
    - Must be manually updated after `member:left` (remove entry) and `member:joined` (add entry), as there will not be an `update:all` event for these cases.
    - For participants, it must be updated after every `update:me` event unless the implementation separates `me` data, in which case `me` will be updated separately and `participants` may catch up with a subsequent `update:all` event.

2.  **`data`: `TournamentData`**

    - Initially populated from `JoinSuccessPayload.data` or `DataSuccessPayload`.
    - Must be updated after every `update:data` event.

3.  **`me`: `ParticipantData | null`** (If stored as a separate object).

    - For participants, initially populated from `participants[JoinSuccessPayload.clientId]` or `MeSuccessPayload`. Must be updated after every `update:me` event.
    - For spectators, this will be `null`.

4.  **`isSpectator`: `boolean`** (shouldn't be stored in state, but derived).
    - Determined client-side after `join:success`.
    - **true** if:
      - The client requested to join with `spectator=true` in the WebSocket connection URL. OR
      - The `JoinSuccessPayload.clientId` is not found in a `participants`.
    - **false** otherwise.

### Additional State Considerations

- `TournamentData.text` and `TournamentData.startedAt` will simultaneously be set (updated to **non null** values) when the tournament starts, via an `update:data` event.
- The server may process multiple successive `type` events from a participant together and reflect the cumulative updates in a single `update:me` event.

### WebSocket Implementation Notes

1.  Abstract WebSocket logic into a class or service from which an instance can be created once and passed around.
2.  Use a single WebSocket connection per client and reuse it for all events.
3.  Reconnect logic should lead to state updates (via the `join:success` event callback) to ensure data consistency.

### Retries

- Reconnecting with the socket after a disconnect should allow the client to recover its previous state and data upon a successful `join:success`.
- Reconnecting logic should always await `join:success` and update relevant client-side state variables.
- All **pollable events** can be retried by the client any number of times.

(**Pollable events** follow the pattern: client fires `{eventName}` and expects the server to respond with `{eventName}:success` or `{eventName}:failure`. They can be implemented as promises that resolve after the server responds.)

---

## Spectator Mode

Clients can join tournaments as non-participating spectators.

### Joining as a Spectator

- Clients include a query parameter `spectator=true` (string "true") in the WebSocket connection URL.
  ```javascript
  const tournamentId = "your_tournament_id";
  const socket = io(namespaceUrl, {
    query: { id: tournamentId, spectator: "true" },
    // ... other options
  });
  ```

### Server-Side Handling of Spectators

- Spectator sockets are added to the same WebSocket **room** as participants to receive broadcast events.
- The server-side logic for spectator sockets **will not have listeners for certain client-sent**, specifically:
  - `type`: Spectators cannot submit typing data.
  - `me`: Spectators do not have individual participant session data to request.
- Consequently, spectators will not receive `update:me` events.
- Spectators **will** receive all other broadcast events (`update:all`, `update:data`, `member:joined`, `member:left`) and can successfully interact with events for which listeners are established for all clients (e.g., `all`, `data`, `check`, `leave`).
- A spectator's `JoinSuccessPayload.clientId` will not be present as a `client.id` in the `JoinSuccessPayload.participants` array.

### Client-Side Guidance for Spectators

- Determine spectator status based on the handshake query and the content of `JoinSuccessPayload`.
- Prevent UI interactions or event emissions for actions not applicable to spectators (e.g., typing input).
- Consider a dedicated view for spectators (e.g., `SpectatorViewTypingArea`) that visually mirrors the participant experience but lacks interactive typing functionality.

---

## Error Codes

All `:failure` events return a `WsFailurePayload` object: `{ code: number; message: string; }`.

### 1xxx: Connection & Handshake Errors

| Code | Event(s)       | Default Message Suggestion                  | Notes                                            |
| ---- | -------------- | ------------------------------------------- | ------------------------------------------------ |
| 1001 | `join:failure` | "Tournament ID missing in handshake query." | `id` query param not provided.                   |
| 1002 | `join:failure` | "Invalid Tournament ID format."             | `id` query param has incorrect format.           |
| 1003 | `join:failure` | "Tournament not found."                     | The specified tournament does not exist.         |
| 1004 | `join:failure` | "Maximum participants reached."             | Tournament is full.                              |
| 1005 | `join:failure` | "Already connected to this tournament."     | If single session per user is enforced.          |
| 1006 | `join:failure` | "Access denied to private tournament."      |                                                  |
| 1007 | `join:failure` | "Spectator mode parameter invalid."         | e.g., if `spectator` param has unexpected value. |

### 2xxx: Client Request & Validation Errors (Primarily for Participants or general requests)

| Code | Event(s)       | Default Message Suggestion                      | Notes                                  |
| ---- | -------------- | ----------------------------------------------- | -------------------------------------- |
| 2001 | _Any_          | "Invalid event name."                           | Server received an unrecognized event. |
| 2002 | _Any_          | "Malformed request payload."                    | e.g., Invalid JSON.                    |
| 2101 | `type:failure` | "Invalid payload: 'character' missing/invalid." | `TypeEventPayload` validation failed.  |
| 2201 | `type:failure` | "Typing not allowed: Tournament not started."   |                                        |
| 2202 | `type:failure` | "Typing not allowed: Tournament ended."         |                                        |
| 2203 | `type:failure` | "Typing not allowed: You have finished."        |                                        |
| 2210 | _Most C2S_     | "Not joined to a tournament."                   | Action requires being in a tournament. |

### 3xxx: Resource & State Errors

| Code | Event(s)                                  | Default Message Suggestion         | Notes                                      |
| ---- | ----------------------------------------- | ---------------------------------- | ------------------------------------------ |
| 3001 | `data:failure`                            | "Tournament data unavailable."     | Error fetching overall tournament details. |
| 3101 | `me:failure` (Participant), `all:failure` | "Participant data unavailable."    | Error fetching participant details.        |
| 3102 | `leave:failure`                           | "Failed to process leave request." |                                            |

### 4xxx: Server-Side Operational Errors

| Code | Event(s) | Default Message Suggestion       | Notes                          |
| ---- | -------- | -------------------------------- | ------------------------------ |
| 4000 | _Any_    | "Internal server error."         | Generic server-side issue.     |
| 4001 | _Any_    | "Operation timed out on server." |                                |
| 4101 | _Any_    | "Database error."                | Problem with data persistence. |

_(This list is not exhaustive and should be expanded as specific error cases are identified during development.)_

---

## Future Improvements

- Add a `cursor` field to `type` event payloads, and resolution logic for common inconsistencies.
- Improve spectator logic by tracking their `ClientSchema` on the server side.
- Add events for managing a visible spectator list on the frontend if this feature is desired (e.g., `spectator:joined`, `spectator:left`, distinct from `member:` events).
- Add an `avatarUrl` or similar attribute to the `UserSchema` for richer user display.
- Define specific tournament capacity limits for participants and spectators.
- Add an analytics api and logic for feeding it on each `type` event for authenticated users to allow it to learn user weaknessess, strengths, etc.
- Add a `TypingAlgorithm` trait or enum that can be chosen during tournament creation and used for typing processing.

---
