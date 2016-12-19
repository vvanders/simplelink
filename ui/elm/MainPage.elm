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

type Source = Sent
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
                        message = {
                            route = packet.route,
                            prn = packet.prn,
                            content = packet.msg,
                            timestamp = 0,
                            source = Received
                        }
                    in
                        ({ model | messages = model.messages ++ [message] }, Cmd.none)
                SimpleLink.Observe(packet) ->
                    let
                        message = {
                            route = packet.route,
                            prn = packet.prn,
                            content = packet.msg,
                            timestamp = 0,
                            source = Observed
                        }
                    in
                        if packet.msg /= "" then --Don't observe acks
                            ({ model | messages = model.messages ++ [message] }, Cmd.none)
                        else
                            (model, Cmd.none)
                SimpleLink.Send(packet) ->
                    let
                        message = {
                            route = packet.route,
                            prn = packet.prn,
                            content = packet.msg,
                            timestamp = 0,
                            source = Sent
                        }
                    in
                       ({ model | messages = model.messages ++ [message] }, Cmd.none)
                SimpleLink.Ack(prn) -> (model, Cmd.none)
                SimpleLink.Retry(prn) -> (model, Cmd.none)
                SimpleLink.Expire(prn) -> (model, Cmd.none)
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

view : Model -> (Msg -> a) -> Html a
view model conv =
    let
        messages = List.map (\msg -> div [] [ text (toString(msg.source) ++ " " ++ (toString msg.prn) ++ ": " ++ msg.content) ]) model.messages
        route = List.indexedMap (\idx addr -> 
            input [ onInput (\str -> conv (UpdateRoute(idx, str))), value addr] []
        ) model.outgoing_route
    in
        div [] [
            div [] messages,
            div [] route,
            input [ onInput (\str -> conv (UpdateMessage str)), value model.outgoing_msg] [],
            button [ onClick (conv (Send model.outgoing_msg)) ] [ text "Send" ]
        ]