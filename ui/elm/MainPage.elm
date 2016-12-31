module MainPage exposing (..)

import SimpleLink
import Html exposing(Html, div, button, text, input)
import Html.Events exposing (onClick, onInput)
import Html.Attributes exposing (value)
import Svg exposing (svg, rect)
import Svg.Attributes exposing (..)
import Time exposing (Time, millisecond)

type alias Model = {
    messages : List Message,
    outgoing_msg : String,
    outgoing_route : List String
}

type alias Pending = {
    retry_count : Int,
    next_retry : Int,
    elapsed_retry : Int,
    elapsed_total : Int
}

type SentStatus = PendingAck(Pending)
    | Acked(Int)
    | Failed(Int)

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
    | Animate(Time)

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
                        pending = PendingAck {
                            retry_count = 0,
                            next_retry = SimpleLink.retry_timeout,
                            elapsed_retry = 0,
                            elapsed_total = 0
                        }
                        message = createMessage packet (Sent pending)
                    in
                       ({ model | messages = model.messages ++ [message] }, Cmd.none)
                SimpleLink.Ack(ack) ->
                    let
                        messages = updateMessage ack.prn
                            (\item -> case item.source of
                                Sent(PendingAck status) -> { item | source = Sent(Acked status.elapsed_total) }
                                _ -> item
                            )
                            model.messages
                    in
                        ({ model | messages = messages }, Cmd.none)
                SimpleLink.Retry(msg) ->
                    let
                        messages = updateMessage msg.prn
                            (\item -> case item.source of
                                Sent(PendingAck(pend)) ->
                                    let
                                        pending = { pend 
                                            | retry_count = pend.retry_count+1,
                                                next_retry = msg.next_retry,
                                                elapsed_retry = 0  }
                                    in
                                        { item | source = Sent(PendingAck(pending)) }
                                _ -> item
                            )
                            model.messages
                    in
                        ({ model | messages = messages }, Cmd.none)
                SimpleLink.Expire(prn) ->
                    let
                        messages = updateMessage prn
                            (\item -> case item.source of
                                Sent(PendingAck status) -> { item | source = Sent(Failed status.elapsed_total) }
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
        Animate(time) -> 
            let
                messages = List.map (\item ->
                    case item.source of
                        Sent(PendingAck status) -> { item 
                            | source = Sent( PendingAck { status 
                                | elapsed_retry = status.elapsed_retry + ceiling (time * millisecond),
                                    elapsed_total = status.elapsed_total + ceiling (time * millisecond) } ) }
                        _ -> item
                ) model.messages
            in
                ({ model | messages = messages }, Cmd.none)

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

getRoutes : List String -> (List String, List String)
getRoutes list =
    let
        index = list
            |> List.indexedMap (\idx item -> if item == "" then Just idx else Nothing)
            |> List.filterMap (\item -> item)
    in
        case List.head index of
            Just idx -> (List.take idx list, List.drop (idx+1) list)
            Nothing -> ([], list)

formatRoute : List String -> Html a
formatRoute route =
    let
        (source, dest) = getRoutes route
        format = (\list ->
            list
            |> List.intersperse " > "
            |> List.foldr String.append ""
        )
        format_source = format source
        format_dest = format dest
    in
        if List.length source > 0 then
            text (format_dest ++ " | " ++ format_source)
        else
            text format_dest

formatMessage : Message -> Html a
formatMessage msg =
    let
        status_color = case msg.source of
            Sent(status) -> case status of
                PendingAck(_) -> "#FFF569"
                Acked(_) -> "#34C92C"
                Failed(_) -> "#DE451B"
            Received -> "#FFFFFF"
            Observed -> "#BABABA"

        status_div = case msg.source of
            Sent(status) ->
                case status of 
                    PendingAck(pending) -> Just (div [ Html.Attributes.style [("width", "100%")] ] [formatRetry pending])
                    Acked(total) -> Just (text ("Received after " ++ (toString total) ++ "ms"))
                    Failed(total) -> Just (text ("Failed to send after " ++ (toString SimpleLink.retry_count) ++ " tries in " ++ (toString total) ++ "ms"))
            _ -> Nothing
        content = ": " ++ msg.content
        route = formatRoute(msg.route)

        main_div = div [Html.Attributes.style [ ("backgroundColor", status_color) ]] [route, text content]
    in
        case status_div of
            Just status ->
                div [Html.Attributes.style [ ("backgroundColor", status_color) ]] [
                    main_div,
                    status
                ]
            Nothing -> main_div
        

formatRetry : Pending -> Html a
formatRetry pending =
    let
        total_pct = toString (((toFloat pending.retry_count) / (toFloat SimpleLink.retry_count)) * 100.0) ++ "px"
        pct = toString (((toFloat pending.elapsed_retry) / (toFloat pending.next_retry)) * 100.0) ++ "px"
    in
        svg [ viewBox "0 0 100 20", width "100%", height "5px", preserveAspectRatio "none"]
            [
                rect [ y "0", width total_pct, height "10px", fill "#0BCEA4"] [],
                rect [ y "10", width pct, height "10px", fill "#0B79CE"] []
            ]