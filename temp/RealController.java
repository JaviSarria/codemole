package com.example.api;

import org.springframework.web.bind.annotation.*;
import com.example.service.ComplexTicketService;

@RestController  
public class RealController {
    
    private ComplexTicketService service;
    
    @PostMapping(value = "TicketCreationXML", consumes = { "application/xml", "application/x-www-form-urlencoded" })
    public String ticketCreation(@RequestBody String xml) {
        System.out.println("Called with: " + xml);
        boolean result = service.crudTicket(xml);
        return result ? "<response>ok</response>" : "<response>fail</response>";
    }
}
