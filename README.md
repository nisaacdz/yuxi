# WebSocket API Specification: Typing Tournament

This document describes the real-time and HTTP APIs as implemented today. It preserves the existing structure and tone while aligning terms, events, and payloads with the Rust code.

## 1. Core Principles

### 1.1. Communication Model

The socket API uses a request-response pattern for client-initiated actions: `eventName` → `eventName:success` or `eventName:failure` (where applicable). The server also pushes room-wide updates.

### 1.2. Identity & Tournament Association (Handshake)

Clients connect to a tournament by including `id` as a query parameter in the WebSocket URL. The server associates the socket with the tournament room identified by that `id`.

Key concepts:

- AuthSchema (server-side): Per-connection context with `user: Option<UserSchema>`.
- TournamentRoomMember (the participant “member”): Has an `id` and optional `user { username }` profile.
- Member ID:
  - Authenticated: Derived from `user.id` (currently a passthrough; see TODO in code for future hardening).
  - Unauthenticated: Generated on first join (UUIDv4) or recovered from the `x-noauth-unique` header.

Joining a tournament:

1. Authenticated participant (default)

- Connect to `namespaceUrl?id=<tournamentId>`.
- Server derives `member.id` from `AuthSchema.user.id`, and `member.user.username = user.username`.

2. Authenticated but anonymous

- Connect to `namespaceUrl?id=<tournamentId>&anonymous=true`.
- Server still derives `member.id` from `AuthSchema.user.id`, but sets `member.user = null` for this session.

3. Unauthenticated participant (always anonymous)

- Connect to `namespaceUrl?id=<tournamentId>`.
- If request includes a valid `x-noauth-unique` header, server decodes it to recover `member.id`.
- If not, server generates a new `member.id` and returns a token in `join:success.noauth`. The client should persist this and send it back via the `x-noauth-unique` header on future HTTP/WebSocket requests.

4. Spectator

- Connect with `spectator=true`: `namespaceUrl?id=<tournamentId>&spectator=true`.
- Spectators are in the room for broadcasts but don’t get participant-only listeners (`type`, `me`). They won’t be added to `participants`.

Connection confirmation:

- On success: `join:success { data, member, participants, noauth }`.
- On failure: `join:failure { code, message }` and the server disconnects.

Client examples:

- Authenticated

```ts
const socket = io(namespaceUrl, { query: { id: tournamentId } });
```

- Authenticated + anonymous

```ts
const socket = io(namespaceUrl, {
  query: { id: tournamentId, anonymous: "true" },
});
```

- Unauthenticated

```ts
const socket = io(namespaceUrl, {
  query: { id: tournamentId },
  extraHeaders: noauth ? { "x-noauth-unique": noauth } : {},
});
```

- Spectator

```ts
const socket = io(namespaceUrl, {
  query: { id: tournamentId, spectator: "true" },
});
```

### 1.3. WebSocket Transport Configuration

Transport order may include `polling`, `websocket`, `webtransport` as needed by the client. Server supports Socket.IO via socketioxide.

### 1.4. Error Handling

All `:failure` payloads are `WsFailurePayload { code: number, message: string }`.

### 1.5. Timing & flow controls

- Join deadline: participants can’t join within 15s of start (`JOIN_DEADLINE = 15s`).
- Inactivity timeout: participant is marked ended after 30s inactivity (`INACTIVITY_TIMEOUT_DURATION`).
- Typing event batching: debounce 200ms, max stack 3, max wait 1000ms.
- `update:all` broadcast: debounce 400ms, max stack 15, max wait 3s.
- Tournament start triggers `update:data` (text + startedAt); end triggers `update:data` (endedAt).

---

## 2. Authentication & Anonymous Participation

### 2.1. Authenticated Members

- Identified by `AuthSchema.user`.
- `member.id` is derived from `user.id`.
- `anonymous=true` masks `member.user` (null) but keeps the same `member.id`.

### 2.2. Unauthenticated Members

- Not logged in; always anonymous.
- Server encodes/decodes a “noauth” token which represents `member.id`.
  - First-time join without header: server creates `member.id` and returns `noauth` in `join:success`.
  - Subsequent requests: client includes `x-noauth-unique` header; server decodes to recover `member.id`.

### 2.3. Live flags in HTTP tournament lists

- For authenticated requests, server uses `user.id`.
- For unauthenticated requests, server tries `x-noauth-unique` to identify `member.id`.
- Result includes live flags: `participating: boolean`, `participantCount: number` and `startedAt`/`endedAt` derived from in-memory state when available.

---

## 3. Client-to-Server Events

Events the client may emit:

- `type` (participants only): `{ character: string }` (one character at a time). Triggers `update:me` and aggregated `update:all`; on error yields `type:failure`.
- `check`: `{}` → `check:success { status: "upcoming" | "started" | "ended" }`.
- `leave`: `{}` → `leave:success` (always sent; spectators are OK to call).
- `me` (participants only): `{}` → `me:success` or `me:failure`.
- `all`: `{}` → `all:success` with all current participants.
- `data`: `{}` → `data:success` with current tournament data.

---

## 4. Server-to-Client Events

### 4.1. Responses to Client Requests

- `join:success` → `JoinSuccessPayload`
- `join:failure` → `WsFailurePayload`
- `me:success` | `me:failure`
- `all:success`
- `leave:success`
- `type:failure`
- `data:success`
- `check:success`

Notes:

- The current server doesn’t emit `all:failure`, `data:failure`, `check:failure`, or `leave:failure`.

### 4.2. Proactive Updates (partial)

- `update:me` (to the participant only) → `UpdateMePayload`
- `update:all` (room broadcast) → `UpdateAllPayload`
- `update:data` (room broadcast) → `UpdateDataPayload`

### 4.3. Broadcast Notifications (full)

- `participant:joined` → `ParticipantJoinedPayload`
- `participant:left` → `ParticipantLeftPayload`

---

## 5. Payload Type Definitions (TypeScript)

These reflect the JSON shapes produced by the Rust serializers (`serde` with `camelCase`).

### 5.1. Core types

```ts
export type UserSchema = {
  id: string;
  username: string;
  email: string;
};

export type TournamentRoomUserProfile = {
  username: string;
};

export type TournamentRoomMember = {
  id: string;
  user: TournamentRoomUserProfile | null;
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

### 5.2. Common payloads

```ts
export type WsFailurePayload = { code: number; message: string };

export type TypeEventPayload = { character: string };
```

### 5.3. Specific payloads

```ts
export type JoinSuccessPayload = {
  data: TournamentData;
  member: TournamentRoomMember;
  participants: ParticipantData[];
  noauth: string; // may be empty when not applicable
};

export type UpdateMePayload = { updates: Partial<ParticipantData> };

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

export type LeaveSuccessPayload = { message: string };
```

---

## 6. Server-Side Implementation Notes

- Namespace: `/`.
- Handshake query params: `id` (required), `spectator` (bool), `anonymous` (bool).
- Header: `x-noauth-unique` (optional, for unauthenticated continuity). CORS explicitly allows this header.
- Rooming: Socket joins room `<tournamentId>` on successful association.
- Lifecycle:
  - Start: when at least one participant exists at scheduled time → set `startedAt`, `scheduledEnd`, generate `text`, emit `update:data`.
  - End: after `scheduledEnd` OR when all participants have finished → set `endedAt`, emit `update:data`, evict manager after 10 minutes.
- Debouncing/batching: `type` input is debounced and aggregated; partial updates (`update:me`, `update:all`) are emitted accordingly.

Code references:

- `api/src/action.rs` (handshake, member resolution, manager bootstrap)
- `app/src/core/manager.rs` (events, payloads, lifecycle, debouncing)
- `api/src/init.rs` (CORS, Socket.IO layer)

---

## 7. Client-Side Notes

- Maintain a single socket instance per tournament.
- Store `noauth` (when provided) and attach as `x-noauth-unique` for future HTTP/WebSocket calls if unauthenticated.
- Participant-local state is driven by `update:me`; list/leaderboard by `update:all` and `participant:joined`/`participant:left`.
- Tournament UI should react to `update:data` for starts/ends.

Suggested state keys:

- `participants: Record<string, ParticipantData>` keyed by `participant.member.id`.
- `me: ParticipantData | null`, `myMemberId: string | null` from `join:success.member.id`.
- `tournament: TournamentData | null`.
- `isSpectator: boolean` inferred from join query and absence from `participants`.

---

## 8. Spectator Mode

Spectators share the room for broadcasts but do not have participant handlers.

- Don’t send `type` or call `me` from spectators; they won’t be handled.
- Spectators still receive: `update:all`, `update:data`, `participant:joined`, `participant:left` and can call `all`, `data`, `check`, `leave`.

---

## 9. Error Codes

Emitted by current server paths (non-exhaustive):

### 1xxx: Connection & Handshake

- 1004 (join:failure): "Tournament no longer accepting participants."
- 1005 (join:failure): "Tournament has already ended."

### 2xxx: Client Request & Validation

- 2210 (type:failure): "Member ID not found." (typing without an active session)

### 3xxx: Resource & State

- 3101 (me:failure): "Your session was not found."

---

## 10. HTTP API (brief)

Base path: `/api/v1`.

- Auth

  - POST `/auth/login` → `{ user, tokens { access } }`
  - POST `/auth/register` → same shape as login
  - GET `/auth/me` → `{ user?: UserSchema }` inside `AuthSchema`
  - POST `/auth/forgot-password`
  - POST `/auth/reset-password`

- Users

  - POST `/users/` → create
  - GET `/users/{id}` → fetch
  - GET `/users/me`, PATCH `/users/me` → current user

- Tournaments
  - GET `/tournaments` → paginated list; query: `page`, `limit`, `privacy`, `status`, `search`
    - Live fields merged when possible: `participating`, `participantCount`, `startedAt`, `endedAt`
    - For unauthenticated clients, include `x-noauth-unique` header to derive participation
  - POST `/tournaments` (auth required) → create
  - GET `/tournaments/{id}` → details

---

## 11. Future Improvements

- Include cursor position in typing payloads.
- Optional spectator roster and events.
- User avatars in `TournamentRoomUserProfile`.
- Capacity limits and enforcement.
- Harden `TournamentRoomMember::get_id` to produce a non-reversible, fixed-format ID.

---
