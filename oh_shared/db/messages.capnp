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
        error            @1 :ErrorResponse;
        # All errors are reported by this sort of messages.
        ok               @2 :OkResponse;
        # Generic OK response, shared by all responses that do not contain
        # more specific response data.

        ping             @3 :PingResponse;
        listDirectory    @4 :ListDirectoryResponse;
        getFile          @5 :GetFileResponse;
        getMatchingFiles @6 :GetMatchingFilesResponse;
        subscribe        @7 :SubscribeResponse;
    }
}

struct ClientRequest {
    id @0 :UInt64;
    union {
        ping             @1  :PingRequest;
        createFile       @2  :CreateFileRequest;
        createFormula    @3  :CreateFormulaRequest;
        createDirectory  @4  :CreateDirectoryRequest;
        removeNode       @5  :RemoveNodeRequest;
        listDirectory    @6  :ListDirectoryRequest;
        getFile          @7  :GetFileRequest;
        getMatchingFiles @8  :GetMatchingFilesRequest;
        setFile          @9  :SetFileRequest;
        setMatchingFiles @10 :SetMatchingFilesRequest;
        subscribe        @11 :SubscribeRequest;
        unsubscribe      @12 :UnsubscribeRequest;
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

struct CreateFileRequest {
    # The parentPath must already exist and have type directory.  The name must
    # not contain /,#,*, or other restricted characters.
    parentPath @0 :Text;
    name @1 :Text;
}

struct CreateFormulaRequest {
    struct Input {
        name @0 :Text;
        path @1 :Text;
    }
    parentPath @0 :Text;
    name @1 :Text;
    inputs @2 :List(Input);
    formula @3 :Text;
}

struct CreateDirectoryRequest {
    parentPath @0 :Text;
    name @1 :Text;
}

struct RemoveNodeRequest {
    # The parentPath must exist and have type directory. A node named |name|
    # must exist in the parentPath's directory.
    parentPath @0 :Text;
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

struct GetFileRequest {
    path @0 :Text;
}
struct GetFileResponse {
    data @0 :Text;
}

struct GetMatchingFilesRequest {
    glob @0 :Text;
}
struct GetMatchingFilesResponse {
    data @0 :List(PathAndData);

    struct PathAndData {
        path @0 :Text;
        data @1 :Text;
    }
}

struct SetFileRequest {
    path @0 :Text;
    data @1 :Text;
}

struct SetMatchingFilesRequest {
    glob @0 :Text;
    data @1 :Text;
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

