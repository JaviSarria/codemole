package com.example.service;

public class TicketCreationService {
    
    public boolean crudTicket(String ticketData) {
        // Calls that should be detected
        Map<String, Object> dataMap = getMapTicketToXML(ticketData);
        
        if (dataMap != null) {
            if (dataMap.containsKey("CREATE")) {
                insertTicket(dataMap);
            } else {
                modificarTicket(dataMap);
            }
        }
        
        return true;
    }
    
    private Map<String, Object> getMapTicketToXML(String xml) {
        Map<String, Object> result = new HashMap<>();
        result.put("data", xml);
        return result;
    }
    
    private void insertTicket(Map<String, Object> data) {
        System.out.println("Inserting: " + data);
    }
    
    private void modificarTicket(Map<String, Object> data) {
        System.out.println("Updating: " + data);
    }
}
