#!/usr/bin/env python2
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from __future__ import print_function, division

import argparse
import audioop
from collections import deque
import ConfigParser
import difflib
import itertools as it
import os
import os.path
import tempfile
import wave
import zmq

import alsaaudio as alsa
try:
    import pocketsphinx as ps
except ImportError:
    # The sphinx is just full of riddles.
    import pocketsphinx as ps


class CaptureSpokenCommands(object):
    # Sphinx expects this format. We always transcribe to this format before storing samples in the internal buffers.
    TranscribeChannels = 1
    TranscribeRate = 16000
    TranscribeFormat = alsa.PCM_FORMAT_S16_LE
    TranscribeFrameSize = 2

    # Constants derived from our parameters above.
    PeriodsPerSecond = 16

    # Ideally we'd subtract the values from a mic across the room, but
    # for now we'll just keep a floating average of "noise".
    # In general this wouldn't work: the floor would reach up with our
    # utterance being part of the noise. However, our recognition phrase
    # is short enough for this not to matter. We'll have to shout, if
    # there has been talking/music, but as talking/music were just
    # happening, this will probably work fine.
    Threshhold = 0.8  # RMS amplitude over the noise floor to recognize an impulse
    NoiseWindowTime = 5  # seconds

    # How long to wait for no-utterance before considering a command completed.
    CommandEndTime = 0.2  # seconds
    CommandEndPeriods = CommandEndTime // (1 / PeriodsPerSecond)

    # Amount of data to keep in the data buffer: e.g. max command length.
    RecordHistoryTime = 5  # seconds

    # Set to true to get noise-floor and record info dumps.
    Debug = False

    def __init__(self, card, prefix, signal_phrases, commands, callback,
                 hmm_directory='/usr/share/pocketsphinx/model/hmm/en_US/hub4wsj_sc_8k',
                 record_rate=TranscribeRate):
        # The native record rate for cases where this is non-native.
        self.record_rate_ = record_rate
        self.record_period_ = self.record_rate_ // self.PeriodsPerSecond

        # Resampling to the transcription rate may be needed if we have to select a different rate.
        self.resample_context_ = None

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
        self.noise_window_ = deque([], maxlen=self.PeriodsPerSecond * self.NoiseWindowTime)

        # The last few seconds of sound data: [period: bytes].
        self.periods_ = deque([], maxlen=self.PeriodsPerSecond * self.RecordHistoryTime)

        # Open the sound card for capture.
        self.pcm = alsa.PCM(alsa.PCM_CAPTURE, alsa.PCM_NORMAL, card)
        self.pcm.setchannels(self.TranscribeChannels)
        self.pcm.setformat(self.TranscribeFormat)
        self.pcm.setrate(self.record_rate_)
        self.pcm.setperiodsize(self.record_period_)
        print("Reading from card: {}".format(self.pcm.cardname()))
        self.pcm.dumpinfo()

        # ASR database for the greeting.
        language_model = os.path.realpath(self.prefix_ + ".lm")
        dictionary = os.path.realpath(self.prefix_ + ".dic")
        sent = os.path.realpath(self.prefix_ + ".sent")
        with open(sent, 'rb') as fp:
            content = fp.read()
            for key in self.commands_.keys():
                assert key in content
        self.decoder_greeting = ps.Decoder(hmm=hmm_directory, lm=language_model, dict=dictionary)

    def read_one_period(self):
        # Wait for next sound samples.
        n_samples, data = self.pcm.read()
        if n_samples != self.record_period_:
            raise Exception("short read")

        # Resample if needed.
        if self.record_rate_ != self.TranscribeRate:
            data, self.resample_context_ = audioop.ratecv(data, self.TranscribeFrameSize, self.TranscribeChannels,
                                                          self.record_rate_, self.TranscribeRate,
                                                          self.resample_context_)

        # Compute rms for this period.
        rms = audioop.rms(data, self.TranscribeFrameSize)

        # Save this frame.
        self.noise_window_.append(rms)
        self.periods_.append(data)
        return rms, data

    def noise_floor(self):
        """Return the current noise floor value based on the noise window."""
        return sum(self.noise_window_) / len(self.noise_window_)

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
        if self.Debug:
            wf = wave.open('/tmp/lastcommand.wav', 'wb')
            wf.setnchannels(self.TranscribeChannels)
            wf.setsampwidth(self.TranscribeFrameSize // self.TranscribeChannels)
            wf.setframerate(self.TranscribeRate)
            wf.writeframes(command_audio)
            wf.close()

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


def main():
    config = ConfigParser.RawConfigParser()
    config.read('/etc/mcp/listener.ini')
    try:
        capture_device = config.get('Config', 'capture_device')
        corpus_prefix = config.get('Config', 'corpus_prefix')
        busted_capture_rate = config.getint('Config', 'busted_capture_rate')
        print(capture_device, corpus_prefix, busted_capture_rate)
    except ConfigParser.NoSectionError:
        capture_device = "default"
        corpus_prefix = "./corpus/corpus"
        busted_capture_rate = CaptureSpokenCommands.TranscribeRate

    parser = argparse.ArgumentParser(description='MCP Command Listener')
    parser.add_argument('--capture-device', '-c', metavar="ALSA_GOOP", default=capture_device,
                        help='ALSA capture device to open.')
    parser.add_argument('--corpus-prefix', '-C', metavar="PREFIX", default=corpus_prefix, type=str,
                        help="Where in the system to find the corpus files.")
    parser.add_argument('--busted-capture-rate', metavar="RATE", default=busted_capture_rate, type=int,
                        help=("Some capture devices do not feature a controllable rate. Sadly we can't easily detect " +
                              "this case with current pyalsaaudio so we take our fixed sample rate on the command " +
                              "line. Use the format data printed on startup to find the right number."))
    args = parser.parse_args()

    ctx = zmq.Context()
    sock = ctx.socket(zmq.PUB)
    sock.bind("tcp://*:31975")

    def on_command(command):
        print("DispatchedCommand: {}".format(command))
        sock.send_json({'command': command})

    commands = {
        'HEY EYRIE TURN ON THE LIGHTS': 'ON',
        'HEY EYRIE TURN THE LIGHTS ON': 'ON',
        'HEY EYRIE TURN OFF THE LIGHTS': 'OFF',
        'HEY EYRIE TURN THE LIGHTS OFF': 'OFF',
        'HEY EYRIE ITS SLEEP TIME': 'SLEEP',
        'HEY EYRIE ITS SLEEPY TIME': 'SLEEP',
        'HEY EYRIE ITS BED TIME': 'SLEEP',
        'HEY EYRIE ITS TIME FOR BED': 'SLEEP',
        'HEY EYRIE ITS TIME TO SLEEP': 'SLEEP',
        'HEY EYRIE TIME TO SLEEP': 'SLEEP',
        'HEY EYRIE LOWER THE LIGHTS': 'LOW',
    }
    listener = CaptureSpokenCommands(args.capture_device, args.corpus_prefix, ["HEY EYRIE", "EYRIE"],
                                     commands, on_command, record_rate=args.busted_capture_rate)
    listener.run()

if __name__ == '__main__':
    main()

