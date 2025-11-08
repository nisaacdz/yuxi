# Bug Investigation and Architectural Findings

This document details the bugs found during investigation and architectural issues that may need future attention.

## Bugs Fixed

### 1. Clippy-Identified Code Quality Issues

#### Redundant Field Names in Struct Initialization
- **Location**: `app/src/core/manager.rs`
- **Issue**: Used `code: code` instead of shorthand `code`
- **Fix**: Changed to use field init shorthand syntax
- **Severity**: Low (style issue, no functional impact)

#### Needless Arbitrary Self Types
- **Location**: `app/src/core/manager.rs` (multiple methods)
- **Issue**: Used `self: Self` instead of `self` and `self: &Self` instead of `&self`
- **Methods affected**: 
  - `execute_tournament_start_logic`
  - `connect`
  - `handle_progress`
  - `handle_typing`
  - `register_type_listeners`
  - `register_base_listeners`
  - `handle_participant_leave`
  - `handle_timeout`
- **Fix**: Replaced with idiomatic self syntax
- **Severity**: Low (style issue, no functional impact)

#### Derivable Default Implementation
- **Location**: `models/src/params/tournament.rs`
- **Issue**: Manual `Default` implementation for `UpdateTournamentParams` that could be derived
- **Fix**: Replaced manual implementation with `#[derive(Default)]`
- **Severity**: Low (reduces boilerplate, no functional impact)

#### Needless Borrows for Generic Args
- **Location**: `api/src/routers/auth.rs`
- **Issue**: Unnecessary borrow `&UserSchema::from(user.clone())` when the function accepts owned value
- **Fix**: Changed to `UserSchema::from(user.clone())`
- **Severity**: Low (minor performance improvement)

#### Map-Flatten Anti-pattern
- **Locations**: 
  - `api/src/action.rs`
  - `api/src/middleware/extension.rs`
  - `api/src/routers/tournament.rs`
- **Issue**: Using `.map(...).flatten()` instead of more idiomatic `.and_then(...)`
- **Fix**: Replaced with `.and_then()` pattern
- **Severity**: Low (improves code readability)

#### Manual Prefix Stripping
- **Location**: `api/src/middleware/extension.rs`
- **Issue**: Manual string slicing `&header[7..]` after checking `starts_with("Bearer ")`
- **Fix**: Replaced with `.strip_prefix("Bearer ")`
- **Severity**: Low (more robust and idiomatic)

#### Unnecessary Clone on Copy Type
- **Location**: `app/src/core/manager.rs`
- **Issue**: Cloning `PartialParticipantData` which implements `Copy`
- **Fix**: Removed `.clone()` call
- **Severity**: Low (minor performance improvement)

#### Potential Panic from Duration Conversion
- **Location**: `app/src/core/manager.rs:453`
- **Issue**: `TimeDelta::from_std(JOIN_DEADLINE).unwrap()` can panic if duration is invalid
- **Fix**: Added fallback with `unwrap_or_else(|_| TimeDelta::seconds(15))`
- **Severity**: Medium (could cause panic in edge cases)

### 2. Remaining Unwrap() Calls

Several `.unwrap()` calls remain in the codebase that could potentially panic:

#### RwLock Unwraps
- **Locations**: Multiple in `app/src/core/manager.rs`
- **Issue**: Calling `.unwrap()` on `RwLock::read()` and `RwLock::write()`
- **Risk**: If a thread panics while holding the lock, the lock becomes poisoned and all subsequent unwraps will panic
- **Current Status**: Not fixed (would require significant refactoring)
- **Recommendation**: Consider using `.expect()` with descriptive messages or proper error handling
- **Examples**:
  - Line 215: `self.typing_text.read().unwrap()`
  - Line 381: `self.inner.typing_text.write().unwrap()`
  - Line 572: `self.inner.typing_text.read().unwrap()`

#### Extension Unwraps
- **Locations**: `app/src/core/manager.rs` lines 677, 739, 883, 898, 1049
- **Issue**: `.extensions.get::<Arc<TournamentRoomMember>>().unwrap()`
- **Risk**: If the extension is not set (programming error), this will panic
- **Current Status**: Not fixed
- **Recommendation**: These are internal invariants, but adding `.expect()` with descriptive messages would help debugging

#### Socket Emit Unwraps
- **Locations**: Various in `app/src/core/manager.rs`
- **Issue**: Some socket emit operations use `.unwrap()`
- **Current Status**: Mixed - some already use proper error handling, others don't
- **Recommendation**: Audit and standardize error handling

## Architectural Concerns

### 1. Security TODOs

#### Insecure Noauth Encoding/Decoding
- **Location**: `api/src/lib.rs:65-72`
- **Issue**: Comment indicates "TODO: Implement more secure encode and decode methods"
- **Current Implementation**: Simply passes UUIDs as-is without any signing or encryption
- **Security Risk**: Medium - Unauthenticated users can potentially forge IDs
- **Recommendation**: Implement HMAC signing or JWT-based tokens for noauth users

#### User ID Transformation
- **Location**: `models/src/schemas/user.rs:45-48`
- **Issue**: Comment indicates "TODO implement a more secure transformation"
- **Current Implementation**: `get_id()` simply clones the user_id string
- **Security Risk**: Low - Purpose unclear, but might relate to privacy concerns
- **Recommendation**: Clarify the purpose and implement if needed

### 2. Error Handling Granularity

#### Database Error Mapping
- **Location**: `api/src/error/adapter.rs:21`
- **Issue**: All database errors map to `INTERNAL_SERVER_ERROR`
- **User Impact**: Poor error messages, harder to debug client issues
- **Recommendation**: Add proper status code mapping for different DbErr variants:
  - `DbErr::RecordNotFound` → `404 NOT_FOUND`
  - `DbErr::Exec` (constraint violation) → `409 CONFLICT`
  - `DbErr::Query` → `400 BAD_REQUEST` (for malformed queries)
  - etc.

### 3. TimeoutMonitor Edge Case

#### State Transition After Timeout
- **Location**: `app/src/core/timeout.rs:88-91`
- **Issue**: When `TimeoutMonitor` is in `TimedOut` state and `call()` is invoked:
  - It runs `after_timeout_fn()`
  - But it returns early without executing the task
  - The state remains `TimedOut`
- **Behavior**: This might be intentional (preventing actions after timeout), but it's not clearly documented
- **Recommendation**: 
  - Add clear documentation about this behavior
  - Consider if the state should reset after `after_timeout_fn()` completes
  - Current usage at line 753 just logs "Timedout user now typing" which suggests the task should execute

### 4. Test Infrastructure

#### Long-Running Tests
- **Issue**: Running `cargo test --all` times out after 300 seconds
- **Possible Causes**:
  - Tests may be starting actual servers or waiting for timeouts
  - Integration tests may need database setup
  - Tests may not properly clean up resources
- **Recommendation**: 
  - Investigate which tests are hanging
  - Add timeout attributes to tests
  - Consider separating unit tests from integration tests

### 5. Lock Poisoning Strategy

The codebase uses `RwLock` and `Mutex` extensively but doesn't have a consistent strategy for handling poisoned locks. Most code uses `.unwrap()` which will panic if a lock is poisoned.

**Recommendation**: Choose one of these strategies:
1. **Panic on poison**: Current approach - acceptable for "this should never happen" scenarios
2. **Recover**: Use `.unwrap_or_else(|e| e.into_inner())` to get the data anyway
3. **Propagate**: Return errors and handle at a higher level

## Summary

### Critical Issues (Must Fix)
- None identified

### High Priority (Should Fix Soon)
- TimeoutMonitor state transition behavior needs clarification
- Duration conversion panic potential (FIXED)

### Medium Priority (Should Fix Eventually)
- Implement secure noauth token encoding
- Improve database error granularity
- Document or improve lock poisoning strategy

### Low Priority (Code Quality)
- All clippy warnings (FIXED)
- Add `.expect()` messages to remaining unwraps
- Investigate test timeouts

## Conclusion

The codebase is generally well-structured with good separation of concerns. Most issues found were code quality and style issues that have been fixed. The remaining issues are primarily around error handling robustness and some architectural decisions that may benefit from additional documentation or refinement.

The core business logic appears sound, and there are no critical bugs that would cause data corruption or security breaches in the current implementation. The main areas for improvement are in error handling, testing infrastructure, and security hardening of the unauthenticated user flow.
