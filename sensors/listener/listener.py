#!/usr/bin/env python3
import alsaaudio as alsa
import audioop
from collections import deque


if __name__ == '__main__':
    # 1024 frames = 64ms per read
    FrameSize = 2
    Rate = 16000
    PeriodSize = 1000
    ReadsPerSecond = Rate // PeriodSize

    pcm = alsa.PCM(alsa.PCM_CAPTURE)
    print(pcm.cardname())
    pcm.setchannels(1)
    pcm.setrate(Rate)
    pcm.setformat(alsa.PCM_FORMAT_S16_LE)
    pcm.setperiodsize(PeriodSize)

    # Ideally we'd subtract the values from a mic across the room, but
    # for now we'll just keep a floating 5-second average of "noise".
    # In general this wouldn't work: the floor would reach up with our
    # utterance being part of the noise. However, our recognition phrase
    # is short enough for this not to matter. We'll have to shout, if
    # there has been talking/music, but as talking/music were just
    # happening, this will probably work fine.
    ThreshHold = 100  # RMS amplitude over the noise floor to recognize an impulse
    RmsFiveSecondWindow = deque([], maxlen=ReadsPerSecond * 5)

    while True:
        nsamples, data = pcm.read()
        if nsamples != PeriodSize:
            raise Exception("short read")

        rms = audioop.rms(data, FrameSize)
        RmsFiveSecondWindow.append(rms)

        fiveSecondAverage = sum(RmsFiveSecondWindow) / len(RmsFiveSecondWindow)

        marker = ""
        if rms - fiveSecondAverage > ThreshHold:
            marker = "*******"

        print(int(fiveSecondAverage), rms, marker)


