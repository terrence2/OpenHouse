function jsonp(path, data, callback) {
    var target = 'http://' + window.location.hostname + ':5000' + path;
    $.ajax({
        url: target,
        data: {data: JSON.stringify(data)},
        dataType: 'jsonp',
        success: callback,
        error: function(xhr, textStatus, errorString) {
            $("#error-display").css('display', 'block')
                               .css('backgroundColor', '#cf0061')
                               .html('A problem has occurred while fetching: ' + target);
        }
    });
}

function matches_all_filters(name, filters) {
    var parts = name.split('-');
    for (var filter of filters) {
        if (filter[0] == '$') {
            if ('$' + parts[0] != filter)
                return false;
        } else if (filter[0] == '@') {
            if ('@' + parts[1] != filter)
                return false;
        } else if (filter[0] == '#') {
            if ('#' + parts[2] != filter)
                return false;
        }
    }
    return true;
}

function ViewModel() {
    var self = this;

    // The full mirrored fs structure.
    self.structure = undefined;

    // The currently selected function.
    self.functionSelect = ko.observable("state");

    // User Control.
    self.userControlValue = ko.observable();

    // Actuators.
    self.actuatorFilter = ko.observable();
    self.actuatorSelection = ko.observableArray();
    ko.computed(function() {
        var raw_filter = self.actuatorFilter();
        if (self.structure === undefined)
            return;

        var filters = raw_filter.split(" ");
        filters = [for (filter of filters) if (filter) filter]
        
        var out = [];

        for (name in self.structure['actuators']['subdirs']) {
            if (matches_all_filters(name, filters))
                out.push({name:name});
        }
        self.actuatorSelection(out);
    });
    self.actuatorPropertyName = ko.observable();  // The property on the actuator to update.
};
var myViewModel = new ViewModel();

function UpdateUserControl() {
    jsonp('/writefiles', {'eyrie/user_control': $('#userControlSelect').val()}, function(data){});
}

function UpdateActuators(property_name, property_value) {
    var data = {};
    for (var actuator of myViewModel.actuatorSelection()) {
        if (myViewModel.structure['actuators']['subdirs'][actuator.name]['subdirs'][property_name])
            data['actuators/' + actuator.name + '/' + property_name] = property_value.toString();
    }
    jsonp('/writefiles', data, function(data){});
}

// Initialize the document with our model.
$(function() {
    jsonp('/structure', {}, function(data) {
            myViewModel.structure = data;
            ko.applyBindings(myViewModel);

            // Poke the filters to force a refresh.
            myViewModel.actuatorFilter("");
        });
    jsonp('/readfiles', ['state'], function(data) {
            var lines = data.data.state.split('\n');
            var current = $.trim(lines.shift());
            lines.shift();
            for (var state of lines) {
                state = $.trim(state);
                $('#userControlSelect').append($('<option>', {value: state}).text(state));
            }
            $('#userControlSelect').val(current);
        });
})
