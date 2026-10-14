// Engine-side simulation helpers kept outside the pinned vendor checkout.
#include "duel.h"

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
