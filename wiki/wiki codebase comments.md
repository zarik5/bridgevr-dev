# Codebase comments

This page contains comments for the codebase that relate to more than one place.

## Sockets efficiency

Using multiple UDP sockets is often unnecessary (https://stackoverflow.com/questions/53573805/does-passing-data-through-multiple-udp-ports-increase-performance) and maybe detrimental (https://discordapp.com/channels/564087419918483486/588170196968013845/644523694051426352).  
In BridgeVR case tightly coupling the packet producers and consumers with one socket each can avoid one memcpy per packet, but one the other hand a decoupled architecture with one or more sockets has the benefit of priority control and simplified memory management.  
Using laminar I need one copy for send and 2 for receive. If I rewrite the receive part so that the receiving end is responsible of creating the buffers, I can achieve 1 copy only

## OpenVR contexts

The "contexts" are the structs given to the OpenVR callbacks and are internally mutable. Using internal mutability allows the callbacks to use the contexts concurrently.

## parking_lot's Mutex

BridgeVR uses parking_lot's mutex because it unlocks itself in case of a thread that holds the lock panics. This reduces the chance of SteamVR noticing the crash and displaying "headset not found" error.

## The request_stop() pattern

Dropping an object that contains a thread loop requires waiting for some actions to timeout. The drops happen sequentially so the time required to execute them is at worst the sum of all timeouts.
By calling request_stop() on all objects involved I can buffer all the shutdown requests at once, so if we drop the objects immediately after, the time needed for all drops is at worst the maximum of all the timeouts.

## BridgeVR bootstrap

To make a minimum system, BridgeVR needs to instantiate VrServer. This means that most OpenVR related settings cannot be changed while the driver is running.
VrServer needs to be instantiated statically because if it get destroyed SteamVR will find invalid pointers.  
Avoid crashing or returning errors, otherwise SteamVR would complain that there is no HMD. If get_settings() returns an error, create the OpenVR server anyway, even if it remains in an unusable state.

## usize in packets

`usize` should never be used in packets because its size is hardware dependent and can cause deserialization to fail. Since Settings is also included in packets, this also applies to settings.
