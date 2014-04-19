#!/usr/bin/env python2
from __future__ import print_function, division

import audioop
import codecs
from collections import deque
import difflib
import itertools as it
import os
import os.path
import tempfile
import wave

import alsaaudio as alsa
try:
    import pocketsphinx as ps
except:
    # The sphinx is just full of riddles.
    import pocketsphinx as ps


class CaptureSpokenCommands:
    # 1024 frames = 64ms per read
    FrameSize = 2
    Rate = 16000
    PeriodSize = 1000
    PeriodsPerSecond = Rate // PeriodSize
    CommandEndPeriods = 5

    # Ideally we'd subtract the values from a mic across the room, but
    # for now we'll just keep a floating 5-second average of "noise".
    # In general this wouldn't work: the floor would reach up with our
    # utterance being part of the noise. However, our recognition phrase
    # is short enough for this not to matter. We'll have to shout, if
    # there has been talking/music, but as talking/music were just
    # happening, this will probably work fine.
    Threshhold = 0.8  # RMS amplitude over the noise floor to recognize an impulse
    NoiseWindowTime = 5  # seconds

    # Amount of data to keep in the data buffer.
    RecordHistoryTime = 5  # seconds

    # Set to true to get noise-floor and record info dumps.
    Debug = False

    def __init__(self, prefix, signal_phrases, commands, callback,
                 hmm_directory='/usr/share/pocketsphinx/model/hmm/en_US/hub4wsj_sc_8k'):
        # We will load |prefix|.lm, |prefix|.dict, and check the commands against |prefix|.sent.
        self.prefix_ = prefix

        # Every command must start with this string.
        self.signal_phrases_ = signal_phrases

        # The set of commands to recognize and the token to
        # pass to the callback.
        self.commands_ = commands  # {str: _}

        # Called when a command is received.
        self.callback_ = callback  # callable(cmd: _) -> None

        # The noise threshhold buffer: [rms: int].
        self.noiseWindow_ = deque([], maxlen=self.PeriodsPerSecond * self.NoiseWindowTime)

        # The last few seconds of sound data: [period: bytes].
        self.periods_ = deque([], maxlen=self.PeriodsPerSecond * self.RecordHistoryTime)

        # Open the sound card for capture.
        self.pcm = alsa.PCM(alsa.PCM_CAPTURE)
        print("Reading from card: {}".format(self.pcm.cardname()))
        self.pcm.setchannels(1)
        self.pcm.setformat(alsa.PCM_FORMAT_S16_LE)
        self.pcm.setrate(self.Rate)
        self.pcm.setperiodsize(self.PeriodSize)

        # ASR database for the greeting.
        langmodel = os.path.realpath(self.prefix_ + ".lm")
        dictionary = os.path.realpath(self.prefix_ + ".dic")
        sent = os.path.realpath(self.prefix_ + ".sent")
        with open(sent, 'rb') as fp:
            content = fp.read()
            for key in self.commands_.keys():
                assert key in content
        self.decoder_greeting = ps.Decoder(hmm=hmm_directory, lm=langmodel, dict=dictionary)

    def read_one_period(self):
        # Wait for next sound samples.
        nsamples, data = self.pcm.read()
        if nsamples != self.PeriodSize:
            raise Exception("short read")
        # Compute rms for this period.
        rms = audioop.rms(data, self.FrameSize)
        # Save this frame.
        self.noiseWindow_.append(rms)
        self.periods_.append(data)
        return rms, data

    def noise_floor(self):
        """Return the current noise floor value based on the noise window."""
        return sum(self.noiseWindow_) / len(self.noiseWindow_)

    def run(self):
        """REPL"""
        while True:
            # Record a prospective commands.
            base_rms = self.wait_for_a_loud_noise()
            command_audio = self.wait_for_the_noise_to_stop(base_rms)
            if command_audio is None:
                continue

            # Transcribe audio -> text.
            command_text = self.transcribe_command(command_audio)
            if not command_text:
                continue

            # Match text -> commands.
            command = self.figure_out_what_was_said(command_text)
            if not command:
                continue

            # Dispatch command.
            self.callback_(command)

    def debug(self, *msg):
        if self.Debug:
            print(*msg)

    def wait_for_a_loud_noise(self):
        """
        Read samples until we hear something.
        """
        while True:
            period_rms, data = self.read_one_period()
            average_rms = self.noise_floor()
            if period_rms - average_rms > (self.Threshhold * average_rms):
                self.debug(int(average_rms), int(period_rms), "\\/ \\/ \\/")
                return average_rms
            else:
                self.debug(int(average_rms), int(period_rms))

    def wait_for_the_noise_to_stop(self, base_rms):
        """
        Read samples until the noise falls back to previous levels or we run through our buffer.
        Returns the command audio as bytes or None on failure.
        """
        # Maximum periods we will record before giving up.
        max_periods = self.RecordHistoryTime * self.PeriodsPerSecond
        num_periods = 0  # Number of periods we have recorded here.
        num_silences = 0  # Number of subsequent silent periods.

        while num_periods < max_periods:
            num_periods += 1

            # Wait for the next period.
            period_rms, data = self.read_one_period()

            # Wait for CommandEndPeriods periods of silence to allow for word breaks and stutters.
            if period_rms - base_rms < 0:
                num_silences += 1
            else:
                num_silences = 0

            if num_silences >= self.CommandEndPeriods:
                self.debug(int(base_rms), int(period_rms), "/\\ /\\ /\\")
                assert num_periods <= len(self.periods_)
                start = len(self.periods_) - num_periods
                frames = it.islice(self.periods_, start, len(self.periods_))
                return ''.join(frames)
            else:
                self.debug(int(base_rms), int(period_rms))

        # If we got here, we never got silence -- probably just a higher
        # baseline now. We should give up.
        return None

    def transcribe_command(self, command_audio):
        fp = tempfile.TemporaryFile()
        fp.write(command_audio)
        fp.seek(0)
        rv = self.decoder_greeting.decode_raw(fp)
        assert rv == len(command_audio) / 2
        best_hyp, _, _ = self.decoder_greeting.get_hyp()
        print("Hypothesis: {}".format(best_hyp))
        return best_hyp

    def figure_out_what_was_said(self, text):
        if not any([text.startswith(phrase) for phrase in self.signal_phrases_]):
            print("No Command: must start with one of {}.".format(self.signal_phrases_))
            return None
        close_matches = difflib.get_close_matches(text, self.commands_)
        if not close_matches:
            print("No Command: no good matches with commands for {}.".format(close_matches))
            return None
        return self.commands_[close_matches[0]]

