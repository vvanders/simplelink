package vvanders.com.simplelink.model;

import vvanders.com.simplelink.SimpleLink;

public class Route {
    public Route(String[] path, int currentLink) {
        Path = path;
        CurrentLink = currentLink;
    }

    public static Route FromLink(int[] path) {
        int count = 0;
        for(int i = 0; i < path.length; ++i) {
            if(path[i] != 0) {
                ++count;
            }
        }

        String[] translatedPath = new String[count];

        int pathIdx = 0;
        int linkIdx = -1;
        for(int i = 0; i < path.length; ++i) {
            if(path[i] == 0) {
                if(linkIdx == -1) {
                    linkIdx = i;
                } else {
                    break;
                }
            } else {
                translatedPath[pathIdx++] = SimpleLink.decode_addr(path[i]);
            }
        }

        return new Route(translatedPath, linkIdx);
    }

    public final String[] Path;
    public final int CurrentLink;
}
