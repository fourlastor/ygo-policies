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
   full observation. JSON null unless Idle, Battle, YesNo, Position or an optional
   Chain. NULL on error; valid until the next call. Optional in older libraries. */
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
