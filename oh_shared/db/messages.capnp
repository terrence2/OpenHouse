@0xe26e34da2a4522fc;

struct ServerMessage {
    union {
        response @0 :ServerResponse;
        event @1 :SubscriptionMessage;
    }
}

struct ServerResponse {
    id @0 :UInt64;
    union {
        error          @1 :ErrorResponse;
        # All errors are reported by this sort of messages.
        ok             @2 :OkResponse;
        # Generic OK response, shared by all responses that do not contain
        # more specific response data.

        ping           @3 :PingResponse;
        listDirectory  @4 :ListDirectoryResponse;
        getFileContent @5 :GetFileContentResponse;
        subscribe      @6 :SubscribeResponse;
    }
}

struct ClientRequest {
    id @0 :UInt64;
    union {
        ping           @1 :PingRequest;
        createNode     @2 :CreateNodeRequest;
        removeNode     @3 :RemoveNodeRequest;
        listDirectory  @4 :ListDirectoryRequest;
        getFileContent @5 :GetFileContentRequest;
        setFileContent @6 :SetFileContentRequest;
        subscribe      @7 :SubscribeRequest;
        unsubscribe    @8 :UnsubscribeRequest;
    }
}

struct PingRequest {
    data @0 :Text;
}

struct PingResponse {
    pong @0 :Text;
}

struct ErrorResponse {
    # All errors contain the error name and some error "context".
    name @0 :Text;
    # The name of the error that occured.
    context @1 :Text;
    # Optional contextual information about the error, or generic information
    # about the error if not specific context is available.
}

struct OkResponse {
    # If the result of a message is boolean true, this is the response.
    # Responses that require more data are specified below their request.
}

struct CreateNodeRequest {
    parentPath @0 :Text;
    # The parent must already exist and have type directory.
    # FIXME: make this multivariate
    nodeType @1 :NodeType;
    name @2 :Text;
    # Must not contain /,#,*, or other restricted characters.

    enum NodeType {
        file @0;
        directory @1;
    }
}

struct RemoveNodeRequest {
    parentPath @0 :Text;
    # FIXME: make this multivariate
    # The parent must already exist, have type directory, and contain |name|.
    name @1 :Text;
}

struct ListDirectoryRequest {
    path @0 :Text;
    # The path must be a single, absolute path. The path must refer to a
    # directory node.
}
struct ListDirectoryResponse {
    children @0 :List(Text);
    # The names of all children stored at the requested path. Note that these
    # are just the names, not complete paths to the children.
}

struct GetFileContentRequest {
    path @0 :Text;
    # The path must be a single, absolute path. The path must refer to a file
    # node.
}
struct GetFileContentResponse {
    data @0 :Text;
    # FIXME: make this data after fixing tree.
}

struct SetFileContentRequest {
    path @0 :Text;
    # FIXME: make this multivariate.

    data @1 :Text;
    # FIXME: this should be Data type, but we need to fix Tree first.
}

enum EventKind {
    created @0;
    removed @1;
    changed @2;
}

struct SubscribeRequest {
    # Register to receive messages whenever the given path changes.
    glob @0 :Text;
}
struct SubscribeResponse {
    subscriptionId @0 :UInt64;
    # An identifier selected by the server that will identify any future
    # subscription messages that were matched by the requested glob.
}
struct SubscriptionMessage {
    subscriptionId @0 :UInt64;
    paths @1 :List(Text);
    kind @2 :EventKind;
    context @3 :Text;
}

struct UnsubscribeRequest {
    # Request to stop receiving messages for the given subscription.
    subscriptionId @0 :UInt64;
    # The identifier must exist.
}

