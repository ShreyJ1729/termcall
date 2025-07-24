from aiortc import RTCPeerConnection
import asyncio


async def main():
    pc = RTCPeerConnection()

    @pc.on("icecandidate")
    async def on_icecandidate(event):
        print("ICE candidate:", event.candidate)

    await pc.setLocalDescription(await pc.createOffer())
    await asyncio.sleep(3)
    await pc.close()


asyncio.run(main())
