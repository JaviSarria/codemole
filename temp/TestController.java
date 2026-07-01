package com.example.api;

import org.springframework.web.bind.annotation.*;
import com.example.service.TicketCreationService;

@RestController
@RequestMapping("/api/tickets")
public class TestController {
    
    private TicketCreationService service;
    
    @PostMapping("/create")
    public String create(@RequestBody String xml) {
        service.crudTicket(xml);
        return "success";
    }
}
