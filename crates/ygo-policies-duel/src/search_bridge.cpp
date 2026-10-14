// Engine-side simulation helpers kept outside the pinned vendor checkout.
#include "duel.h"
#include "card.h"
#include "effect.h"

// Only call on a simulated branch after taking a snapshot. Restoring that
// snapshot also restores the live random generator: search never gets to see
// the live future coin tosses, dice rolls or engine shuffles.
extern "C" int YGO_DuelReseed(void* handle, uint64_t seed) {
    if(!handle)
        return 0;
    auto* game = static_cast<duel*>(handle);
    RNG::Xoshiro256StarStar::StateType state{};
    for(auto& word : state) {
        seed += UINT64_C(0x9e3779b97f4a7c15);
        uint64_t z = seed;
        z = (z ^ (z >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)) * UINT64_C(0x94d049bb133111eb);
        word = z ^ (z >> 31);
    }
    game->reseed_for_search(state);
    return 1;
}

// An initial effect descriptor contains only declarations made by a card's
// initial_effect function. It is not an activation check: no condition, cost,
// target, value or operation function is called to obtain these fields.
// Keep this layout in sync with InitialEffect in core.rs.
struct YGO_InitialEffect {
    uint64_t category;
    uint64_t description;
    uint64_t flags;
    uint32_t event;
    uint16_t effect_type;
    uint16_t range;
    uint8_t has_cost;
    uint8_t card_target;
};

// Rust only calls this on a fresh, unstarted metadata duel. Creating the card
// directly performs exactly the same script initialization as DuelNewCard,
// without placing it on a live field or running the field adjustment loop.
extern "C" int YGO_DuelInitialEffects(
        void* handle, uint32_t code, void* output,
        void (*append)(void*, const YGO_InitialEffect*)) {
    if(!handle || !code || !append)
        return 0;
    auto* game = static_cast<duel*>(handle);
    duel_arena_scope scope(game->arena);
    try {
        auto* definition = game->new_card(code);
        for(const auto* container : {&definition->single_effect,
                &definition->field_effect, &definition->equip_effect,
                &definition->target_effect, &definition->xmaterial_effect}) {
            for(const auto& entry : *container) {
                const auto* effect = entry.second;
                YGO_InitialEffect profile{};
                profile.category = effect->category;
                profile.description = effect->description;
                profile.flags = uint64_t(effect->flag[0]) | (uint64_t(effect->flag[1]) << 32);
                profile.event = effect->code;
                profile.effect_type = effect->type;
                profile.range = effect->range;
                profile.has_cost = effect->cost != 0;
                profile.card_target = effect->is_flag(EFFECT_FLAG_CARD_TARGET);
                append(output, &profile);
            }
        }
        return 1;
    } catch(...) {
        return 0;
    }
}
