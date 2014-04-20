# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from django.conf.urls import patterns, url

from graph import views

urlpatterns = patterns('',
    url(r'^$', views.index, name='index'),
    url(r'^summary/', views.summary, name='summary'),
    #url(r'^diff/$', views.diff, name='diff'),
    #url(r'^result/$', views.result, name='result'),
)
