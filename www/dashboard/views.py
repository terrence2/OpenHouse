# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from django.shortcuts import render
from django.template import loader


def index(request):
    return render(request, "dashboard.html")
