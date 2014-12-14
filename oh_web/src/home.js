function Connection(sock) {
    this.socket = sock;
}
Connection.prototype.ping = function(message) {
    var conn = this;
    return new Promise(function(accept, reject) {
        conn.socket.onmessage = function(e) {
            var msg = JSON.parse(e.data);
            if (msg['error'] !== undefined)
                reject(msg);
            accept(msg);
        };
        conn.socket.send(JSON.stringify({type: 'ping', ping: message}));
    });
};
Connection.prototype.query = function(q) {
    var conn = this;
    return new Promise(function(accept, reject) {
        conn.socket.onmessage = function(e) {
            var msg = JSON.parse(e.data);
            if (msg['error'] !== undefined)
                reject(msg);
            accept(msg);
        };
        var query = {query: q, transforms: []};
        var msg = {type: 'query', query_group: [query]};
        conn.socket.send(JSON.stringify(msg));
    });
};

function connect(address) {
    return new Promise(function(accept, reject) {
        var socket = new WebSocket(address, "JSON");

        socket.onopen = function(e) {
            accept(new Connection(socket));
        }
    });
}

exports.connect = connect;
