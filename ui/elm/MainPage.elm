module MainPage exposing (..)

import SimpleLink
import Html exposing(Html, div, button, text, input)
import Html.Events exposing (onClick, onInput)
import Html.Attributes exposing (value)

type alias Model = {
    messages : List Message,
    outgoing_msg : String,
    outgoing_route : List String
}

type SentStatus = PendingAck(Int)
    | Acked
    | Failed

type Source = Sent(SentStatus)
    | Received
    | Observed

type alias Message = {
    route : List String,
    prn : SimpleLink.PRN,
    content : String,
    timestamp : Int,
    source : Source
}

type Msg = Link(SimpleLink.Msg)
    | Send(String)
    | UpdateMessage(String)
    | UpdateRoute(Int, String)

init : Model
init = {
        messages = [],
        outgoing_msg = "",
        outgoing_route = [""]
    }

update : Msg -> Model -> (SimpleLink.SendMsg -> Cmd msg) -> (Model, Cmd msg)
update msg model send = 
    case msg of
        Link(action) -> 
            case action of
                SimpleLink.Recv(packet) ->
                    let
                        message = createMessage packet Received
                    in
                        ({ model | messages = model.messages ++ [message] }, Cmd.none)
                SimpleLink.Observe(packet) ->
                    let
                        message = createMessage packet Observed
                    in
                        if packet.msg /= "" then --Don't observe acks
                            ({ model | messages = model.messages ++ [message] }, Cmd.none)
                        else
                            (model, Cmd.none)
                SimpleLink.Send(packet) ->
                    let
                        message = createMessage packet (Sent(PendingAck 0))
                    in
                       ({ model | messages = model.messages ++ [message] }, Cmd.none)
                SimpleLink.Ack(ack) ->
                    let
                        messages = updateMessage ack.prn
                            (\item -> case item.source of
                                Sent(_) -> { item | source = Sent(Acked) }
                                _ -> item
                            )
                            model.messages
                    in
                        ({ model | messages = messages }, Cmd.none)
                SimpleLink.Retry(prn) ->
                    let
                        messages = updateMessage prn
                            (\item -> case item.source of
                                Sent(PendingAck(count)) -> { item | source = Sent(PendingAck(count+1)) }
                                _ -> item
                            )
                            model.messages
                    in
                        ({ model | messages = messages }, Cmd.none)
                SimpleLink.Expire(prn) ->
                    let
                        messages = updateMessage prn
                            (\item -> case item.source of
                                Sent(_) -> { item | source = Sent(Failed) }
                                _ -> item
                            )
                            model.messages
                    in
                        ({ model | messages = messages }, Cmd.none)
        UpdateMessage(str) -> ({ model | outgoing_msg = str }, Cmd.none)
        UpdateRoute(idx, str) ->
            let
                append = case List.reverse model.outgoing_route |> List.head of
                    Just end ->
                        --Only append if we have a full callsign and less than 15 total routes
                        if String.length end >= 5 && List.length model.outgoing_route < 15 then [""] else []
                    _ -> []
                route = List.indexedMap (\midx mstr -> if midx == idx then str else mstr) model.outgoing_route
            in
                ({ model | outgoing_route = route ++ append }, Cmd.none)
        Send(str) ->
            let
                msg = {
                    route = List.filter (\addr -> String.length addr > 0) model.outgoing_route,
                    prn = 0,
                    msg = str
                }
            in
                ({ model | outgoing_msg = "" }, send msg)

createMessage : SimpleLink.RecvMsg -> Source -> Message
createMessage packet source =
    {
        route = packet.route,
        prn = packet.prn,
        content = packet.msg,
        timestamp = 0,
        source = source
    }

updateMessage : SimpleLink.PRN -> (Message -> Message) -> List Message -> List Message
updateMessage prn update messages =
    List.map (\item ->
        if item.prn == prn then 
            update item 
        else
            item
    )
    messages

view : Model -> (Msg -> a) -> Html a
view model conv =
    let
        messages = List.map (\msg -> formatMessage msg) model.messages
        route = List.indexedMap (\idx addr -> 
            input [ onInput (\str -> conv (UpdateRoute(idx, str))), value addr] []
        ) model.outgoing_route
    in
        div [] [
            div [] messages,
            div [] ([text "Route: "] ++ route),
            input [ onInput (\str -> conv (UpdateMessage str)), value model.outgoing_msg] [],
            button [ onClick (conv (Send model.outgoing_msg)) ] [ text "Send" ]
        ]

formatRoute : List String -> Html a
formatRoute route =
    route 
        |> List.filter (\r -> r /= "0000000") 
        |> List.foldl (\l r -> l ++ " -> " ++ r) ""
        |> text

formatMessage : Message -> Html a
formatMessage msg =
    let
        source = case msg.source of
            Sent(status) -> case status of
                PendingAck(count) -> "P " ++ (toString count)
                Acked -> "S"
                Failed -> "F"
            Received -> "R"
            Observed -> "O"
        content = " - " ++ (toString msg.prn) ++ ": " ++ msg.content
        route = formatRoute(msg.route)
    in
        div [] [text (source ++ " "), route, text content]