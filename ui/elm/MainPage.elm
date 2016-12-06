module MainPage exposing (..)

import SimpleLink
import Html exposing(Html, div, button, text, input)
import Html.Events exposing (onClick, onInput)

type alias Model = {
    messages : List Message,
    outgoing_msg : String,
    outgoing_route : List String
}

type alias Message = {
    route : List String,
    prn : SimpleLink.PRN,
    content : String,
    timestamp : Int
}

type Msg = Link(SimpleLink.Msg)
    | Send(String)
    | UpdateMessage(String)

init : Model
init = {
        messages = [],
        outgoing_msg = "",
        outgoing_route = []
    }

update : Msg -> Model -> (Model, Cmd Msg)
update msg model = 
    case msg of
        Link(action) -> 
            case action of
                SimpleLink.Recv(packet) ->
                    let
                        message = {
                            route = packet.route,
                            prn = packet.prn,
                            content = packet.msg,
                            timestamp = 0
                        }
                    in
                        ({ model | messages = model.messages ++ [message] }, Cmd.none)
                SimpleLink.Observe(packet) -> (model, Cmd.none)
                SimpleLink.Ack(prn) -> (model, Cmd.none)
                SimpleLink.Retry(prn) -> (model, Cmd.none)
                SimpleLink.Expire(prn) -> (model, Cmd.none)
        UpdateMessage(str) -> ({ model | outgoing_msg = str }, Cmd.none)
        Send(str) -> (model, Cmd.none)

view : Model -> (Msg -> a) -> Html a
view model conv =
    div [] [
        input [ onInput (\str -> conv (UpdateMessage str)) ] [],
        button [ onClick (conv (Send model.outgoing_msg)) ] [ text "Send" ]
    ]