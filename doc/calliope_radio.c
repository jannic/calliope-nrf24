/*
Program used to send data from Calliope mini
*/

#define _GNU_SOURCE

#include "MicroBit.h"
#include "NEPODefs.h"
#include <list>
#include <array>
#include <stdlib.h>
MicroBit _uBit;


double ___cnt;

int main()
{
    _uBit.init();
    ___cnt = 0;
    
    _uBit.radio.enable();
    _uBit.radio.setGroup(7);
    _uBit.display.scroll(ManagedString("run"));
    while ( true ) {
        _uBit.radio.setTransmitPower(7);
        _uBit.radio.datagram.send(ManagedString((int)(___cnt)));
        ___cnt = ___cnt + 1;
        _uBit.sleep(_ITERATION_SLEEP_TIMEOUT);
    }
    release_fiber();
}
