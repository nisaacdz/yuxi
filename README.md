# WebSocket API Specification: Typing Tournament

This document describes the real-time and HTTP APIs as implemented in the current codebase. It aligns all terms, events, and payloads with the authoritative Rust implementation.

## 1. Core Principles

### 1.1. Communication Model

The API follows a standard request-response pattern for client-initiated actions (`eventName` → `eventName:success` or `eventName:failure`). The server also pushes unsolicited, room-wide updates to all connected clients.

### 1.2. Identity & Tournament Association (Handshake)

A client connects to a specific tournament by providing the tournament `id` as a query parameter in the WebSocket connection URL. The server then associates that socket connection with the corresponding tournament "room".

**Key Identity Concepts:**

*   **AuthSchema (Server-Side):** Represents the connection's authentication context, containing an `Option<UserSchema>`.
*   **TournamentRoomMember:** The canonical representation of a participant within a tournament. It has a stable `id` and an optional `user` profile.
*   **Member ID:**
    *   **Authenticated User:** The `member.id` is derived from the authenticated `user.id`.
    *   **Unauthenticated User:** The `member.id` is a UUIDv4 generated on the first connection. This ID can be recovered on subsequent connections if the client provides the `x-noauth-unique` header.

**Joining a Tournament:**

1.  **Authenticated Participant (Default):**
    *   Connect to: `namespaceUrl?id=<tournamentId>`
    *   The server derives `member.id` from the user's JWT and populates `member.user` with their profile.

2.  **Authenticated but Anonymous:**
    *   Connect to: `namespaceUrl?id=<tournamentId>&anonymous=true`
    *   The server uses the authenticated `user.id` to derive `member.id` but sets `member.user` to `null` to preserve anonymity for the session.

3.  **Unauthenticated Participant (Always Anonymous):**
    *   Connect to: `namespaceUrl?id=<tournamentId>`
    *   If the request includes a valid `x-noauth-unique` header, the server decodes it to recover the existing `member.id`.
    *   Otherwise, the server generates a new `member.id` and returns a token in the `join:success.noauth` payload. The client **should** persist this token and send it back in the `x-noauth-unique` header on all future WebSocket and HTTP requests to maintain a consistent identity.

4.  **Spectator:**
    *   Connect to: `namespaceUrl?id=<tournamentId>&spectator=true`
    *   Spectators are placed in the tournament room to receive broadcasts but are not considered participants. They cannot emit participant-only events like `type`.

**Connection Confirmation:**

*   **On Success:** The server emits a `join:success` event with the initial state.
*   **On Failure:** The server emits a `join:failure` event with an error code and message, then immediately disconnects the socket.

### 1.3. WebSocket Transport Configuration

> **Important: Connection must start with HTTP Polling**
>
> The client-side Socket.IO transport configuration **must** include `polling` as the first option (e.g., `transports: ["polling", "websocket"]`). This is a mandatory requirement for two primary reasons:
>
> 1.  **Middleware Execution:** The server is built on Axum, and critical logic (authentication, CORS, logging) is implemented as standard HTTP middleware. The initial connection must be an HTTP request to ensure this middleware chain is executed correctly.
> 2.  **Custom Headers:** Authentication (`Authorization`) and anonymous identity (`x-noauth-unique`) are passed via custom HTTP headers. The WebSocket protocol itself does not support custom headers after the initial handshake. Starting with HTTP polling ensures these headers are received and processed by the server before the connection is upgraded to a persistent WebSocket.

### 1.4. Error Handling

All `:failure` payloads conform to the `WsFailurePayload { code: number, message: string }` structure.

### 1.5. Timing & Flow Controls

The server manages event flow to ensure efficiency and prevent abuse:

*   **Join Deadline:** Participants are prevented from joining a tournament that is about to start or has already started.
*   **Inactivity Timeout:** If a participant stops sending `type` events for a specific duration while the tournament is active, the server will automatically mark their session as finished.
*   **Typing Event Batching:** To reduce network traffic, individual character inputs from the `type` event are buffered and processed in batches on the server. This is managed by a combination of debouncing (waiting for a pause in typing), a stack size limit (processing after N characters), and a maximum wait time (processing after a certain time has passed, regardless of activity).
*   **Broadcast Throttling:** The `update:all` event, which sends data about all participants, is throttled to avoid flooding clients with messages during periods of high activity.

---

## 2. Optimistic Updates & Client-Side Prediction

The API is designed to support a highly responsive user experience through **Optimistic Updates** (also known as Client-Side Prediction). This is achieved using a Request ID (`rid`).

When the client sends a `type` event, it must include a unique, monotonically increasing `rid`. The server will process the event and then include the *same* `rid` in the corresponding `update:me` payload.

**Recommended Client Implementation:**

1.  **Local Update:** When the user types a character, immediately update the local UI (e.g., advance the cursor, recalculate WPM). Do not wait for the server.
2.  **Send Event:** Generate a new `rid` and send the `type` event with the typed character and the `rid`.
3.  **Receive Confirmation:** When the `update:me` event arrives from the server, check its `rid`.
4.  **Reconcile State:** If the incoming `rid` matches the last `rid` you sent, you can trust the server's payload as the authoritative state and replace your locally predicted state with it. This corrects any minor discrepancies (e.g., in WPM calculations) between the client and server.

This flow makes the UI feel instantaneous while still ensuring eventual consistency with the server's state.

---

## 3. Client-to-Server Events

Events the client may emit:

*   `type` (Participants only): `{ character: string, rid: number }`. Triggers an `update:me` response and contributes to the throttled `update:all` broadcast.
*   `check`: `{}` → `check:success { status: "upcoming" | "started" | "ended" }`.
*   `leave`: `{}` → `leave:success`.
*   `me` (Participants only): `{}` → `me:success` or `me:failure`.
*   `all`: `{}` → `all:success` with data for all current participants.
*   `data`: `{}` → `data:success` with the current state of the tournament.

---

## 4. Server-to-Client Events

### 4.1. Responses to Client Requests

*   `join:success` | `join:failure`
*   `me:success` | `me:failure`
*   `all:success`
*   `leave:success`
*   `type:failure`
*   `data:success`
*   `check:success`

### 4.2. Proactive Server Updates

*   `update:me` (To the originating participant only) → `UpdateMePayload`. Confirms a `type` event.
*   `update:all` (Room broadcast) → `UpdateAllPayload`. Throttled broadcast of all participant states.
*   `update:data` (Room broadcast) → `UpdateDataPayload`. Sent when core tournament data changes (e.g., it starts or ends).

### 4.3. Broadcast Notifications

*   `participant:joined` (Room broadcast) → `ParticipantJoinedPayload`.
*   `participant:left` (Room broadcast) → `ParticipantLeftPayload`.

---

## 5. Payload Type Definitions (TypeScript)

These definitions reflect the JSON shapes produced by the Rust serializers.

### 5.1. Core Types

```ts
export type TournamentRoomUserProfile = {
  username: string;
};

export type TournamentRoomMember = {
  id: string;
  user: TournamentRoomUserProfile | null;
  participant: boolean;
};

export type ParticipantData = {
  member: TournamentRoomMember;
  currentPosition: number;
  correctPosition: number;
  totalKeystrokes: number;
  currentSpeed: number; // WPM (rounded)
  currentAccuracy: number; // % (rounded)
  startedAt: string | null;
  endedAt: string | null;
};

export type TournamentData = {
  id: string;
  title: string;
  createdAt: string;
  createdBy: string;
  scheduledFor: string;
  description: string;
  startedAt: string | null;
  endedAt: string | null;
  scheduledEnd: string | null;
  text: string | null;
};
```

### 5.2. Event Payloads

```ts
export type WsFailurePayload = { code: number; message: string };

// Client -> Server
export type TypeEventPayload = {
  character: string;
  rid: number; // Request ID for optimistic updates
};

// Server -> Client
export type JoinSuccessPayload = {
  data: TournamentData;
  member: TournamentRoomMember;
  participants: ParticipantData[];
  noauth: string; // May be empty if authenticated
};

// Server -> Client
export type UpdateMePayload = {
  updates: Partial<ParticipantData>;
  rid: number; // Mirrors the rid from the triggering `type` event
};

export type PartialParticipantDataForUpdate = {
  memberId: string;
  updates: Partial<ParticipantData>;
};

export type UpdateAllPayload = { updates: PartialParticipantDataForUpdate[] };

export type UpdateDataPayload = {
  updates: Partial<Omit<TournamentData, "id" | "createdAt" | "createdBy">>;
};

export type CheckSuccessPayload = { status: "upcoming" | "started" | "ended" };

export type ParticipantJoinedPayload = { participant: ParticipantData };

export type ParticipantLeftPayload = { memberId: string };
```

---

## 6. Server-Side Implementation Notes

*   **Namespace:** All events operate on the root `/` namespace.
*   **Handshake Query Params:** `id` (required), `spectator` (boolean), `anonymous` (boolean).
*   **Handshake Header:** `x-noauth-unique` is used to maintain identity for unauthenticated users.
*   **Lifecycle:**
    *   **Start:** A tournament officially starts at its `scheduled_for` time if at least one participant is present. This generates the typing `text`, sets `startedAt`, and broadcasts an `update:data` event.
    *   **End:** A tournament ends when its scheduled duration expires or when all participants have either finished or timed out due to inactivity. This sets `endedAt` and broadcasts a final `update:data` event. The manager instance is evicted from memory after a grace period.

---

## 7. Error Codes

A non-exhaustive list of error codes emitted by the server:

### 1xxx: Connection & Handshake

*   `1004` (on `join:failure`): "Tournament no longer accepting participants."
*   `1005` (on `join:failure`): "Tournament has already ended."

### 2xxx: Client Request & Validation

*   `2210` (on `type:failure`): "Member ID not found." (Sent if a user types without a valid session).
*   `2211` (on `type:failure`): "Your session has ended." (Sent if a user types after finishing).

### 3xxx: Resource & State

*   `3101` (on `me:failure`): "Your session was not found."