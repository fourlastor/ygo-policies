/* C ABI of libygo_policies (crates/ygo-policies-ffi).
 *
 * One handle is one duel seat.  Feed it every OCGCore message, in order, as
 * OCG_DuelGetMessage returns them (unfiltered: the library hides from the
 * seat whatever that player may not see).  When a message asks the seat to
 * decide, the feed call returns 1 and ygo_policy_response() holds the bytes to
 * pass to OCG_DuelSetResponse.
 *
 * Optional but recommended, as EDOPro's server does:
 *   - before the first engine message, MSG_START:
 *       u8 4, u8 seat, u32 lp0, u32 lp1, u16 deck0, u16 extra0, u16 deck1, u16 extra1
 *   - MSG_UPDATE_DATA for the Extra Decks right after it, and for the hand, field,
 *     Graveyard and banished cards before each prompt of this seat:
 *       u8 6, u8 player, u8 location, <OCG_DuelQueryLocation result>
 *     (query flags 0x120333: code, position, level, rank, ATK, DEF, counters, public)
 */
#ifndef YGO_POLICIES_H
#define YGO_POLICIES_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct YgoPolicy YgoPolicy;

/* NULL on failure (see ygo_policy_last_error).  `seat` is 0 (goes first) or 1,
   or -1 to learn it from MSG_START.  `cards_cdb_path` is an OCGCore cards.cdb. */
YgoPolicy *ygo_policy_create(const char *policy_id, const char *cards_cdb_path, int32_t seat);

/* Like ygo_policy_create; `seed` seeds tie-breaking between equally good
   choices (which face-down card to destroy, which zone to use).  The same seed
   and message stream give the same answers.  ygo_policy_create uses seed 0. */
YgoPolicy *ygo_policy_create_seeded(const char *policy_id, const char *cards_cdb_path, int32_t seat,
                                    uint64_t seed);

/* Independent copy, including pending response, policy memory and RNG. NULL on
   failure; destroy the copy with ygo_policy_destroy. Optional in older libraries. */
YgoPolicy *ygo_policy_clone(const YgoPolicy *policy);

/* Enable state-safe replacement of future responses, before feeding the first
   selection prompt. Forks inherit this setting. 0 on success, -1 on error.
   Optional in older libraries; ordinary seats avoid the snapshot overhead. */
int32_t ygo_policy_enable_response_overrides(YgoPolicy *policy);

/* Stop override checkpoints after committing the final searched response.
   Preserves the committed policy, RNG and pending answer. Search tails using
   this API must reject MSG_RETRY instead of feeding it. 0 on success, -1 on
   error. Optional in older libraries. */
int32_t ygo_policy_disable_response_overrides(YgoPolicy *policy);

/* Install public printed effect descriptors, keyed by card code, before the
   first selection prompt. Forks inherit them. JSON never contains live hidden
   state. 0 on success, -1 on invalid JSON/unsupported policy/late replacement.
   Optional in older libraries. */
int32_t ygo_policy_set_effect_profiles_json(YgoPolicy *policy, const char *profiles_json);

/* Replace the pending response with another offered response and commit its
   action to policy memory. The pending decision must have been fed after
   enabling overrides. Sequential alternatives must first be expanded below.
   0 on success, -1 on error; errors preserve the response. Optional API. */
int32_t ygo_policy_override_response(YgoPolicy *policy, const uint8_t *response, size_t length);

/* Expand a sequential target/material prompt to at most twelve legal whole
   responses, cache each resulting policy state, and return updated last-answer
   JSON. `maximum` is clamped to 1..12; at most that many initial candidates are
   attempted. Choices and responses remain aligned. Other prompts are unchanged.
   NULL on error; valid until the next call. Optional in older libraries. */
const char *ygo_policy_alternative_responses_json(YgoPolicy *policy, size_t maximum);

/* One message (message id byte + body).  1: this seat must answer (see
   ygo_policy_response); 0: nothing to answer; -1: error. */
int32_t ygo_policy_feed(YgoPolicy *policy, const uint8_t *message, size_t length);

/* Decode one MSG_UPDATE_DATA/CARD for two distinct handles of this library.
   0 on success, -1 on error. Optional in older libraries. */
int32_t ygo_policy_feed_update_pair(YgoPolicy *first, YgoPolicy *second,
                                  const uint8_t *message, size_t length);

/* A whole OCG_DuelGetMessage buffer (u32 length + message, repeated). */
int32_t ygo_policy_feed_buffer(YgoPolicy *policy, const uint8_t *buffer, size_t length);

/* The response of the last feed that returned 1; valid until the next call on this handle. */
const uint8_t *ygo_policy_response(const YgoPolicy *policy, size_t *length);

/* The seat (0 or 1), or -1 while unknown. */
int32_t ygo_policy_seat(const YgoPolicy *policy);

/* JSON of what the seat currently sees, and of its last decision
   ({"observation", "decision", "choice", "responses"}); valid until the next call. */
const char *ygo_policy_observation_json(YgoPolicy *policy);
const char *ygo_policy_last_answer_json(YgoPolicy *policy);

/* Compact search view: {decision, choice, responses, situation}, without the
   full observation. Supports Idle, Battle, YesNo, Position, Option, card/material
   selections, or optional Chain; JSON null otherwise. NULL on error; valid until
   the next call. Optional in older libraries. */
const char *ygo_policy_search_view_json(YgoPolicy *policy);

void ygo_policy_destroy(YgoPolicy *policy);

/* The last error on this thread, or NULL. */
const char *ygo_policy_last_error(void);

/* JSON array of {"id", "deck"}: every policy and the deck it pilots (see decks/). */
const char *ygo_policy_catalog(void);

#ifdef __cplusplus
}
#endif

#endif
