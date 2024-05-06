#pragma once

#include "Config.h"

#if USE_NANOSIGNALSLOT
#include "SignalSlot/nano_signal_slot.hpp"
#define SIGNAL Nano::Signal
#else
#include "Sigs.h"
#define SIGNAL sigs::Signal
#endif 

